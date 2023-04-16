use std::ops::{Bound, Index, Range, RangeBounds};

use crate::cache::{Cache, CachedMacroCellBranch};

pub struct StateBuffer {
    rows: usize,
    cols: usize,
    state: Vec<bool>,
}

impl StateBuffer {
    pub fn new(state: Vec<bool>, rows: usize, cols: usize) -> Self {
        Self { rows, cols, state }
    }

    pub fn view(&self) -> StateBufferView {
        StateBufferView::new(&self.state, self.rows, self.cols)
    }
}

#[derive(Clone, Copy)]
pub struct StateBufferView<'a> {
    rows: usize,
    cols: usize,
    row_stride: usize,
    view: &'a [bool],
}

fn normalize_range<R: RangeBounds<usize>>(range: R, start: usize, end: usize) -> Range<usize> {
    let norm_start = match range.start_bound() {
        Bound::Included(&i) => i,
        Bound::Excluded(&i) => i + 1,
        Bound::Unbounded => start,
    };
    let norm_end = match range.end_bound() {
        Bound::Included(&i) => i + 1,
        Bound::Excluded(&i) => i,
        Bound::Unbounded => end,
    };
    assert!(norm_start >= start && norm_end <= end && norm_start <= norm_end);
    norm_start..norm_end
}

impl<'a> StateBufferView<'a> {
    pub fn new(buffer: &'a [bool], rows: usize, cols: usize) -> Self {
        assert_eq!(buffer.len(), rows * cols);
        Self {
            rows,
            cols,
            row_stride: cols,
            view: buffer,
        }
    }

    pub fn sub_rectangle<R: RangeBounds<usize>, C: RangeBounds<usize>>(
        &self,
        rows: R,
        cols: C,
    ) -> Self {
        let rows = normalize_range(rows, 0, self.rows);
        let cols = normalize_range(cols, 0, self.cols);
        Self {
            rows: rows.end - rows.start,
            cols: cols.end - cols.start,
            row_stride: self.row_stride,
            view: &self.view[rows.start * self.row_stride + cols.start..],
        }
    }

    pub fn rows(&self) -> usize {
        self.rows
    }

    pub fn cols(&self) -> usize {
        self.cols
    }
}

impl<'a> Index<(usize, usize)> for StateBufferView<'a> {
    type Output = bool;

    fn index(&self, (row, col): (usize, usize)) -> &Self::Output {
        assert!(row < self.rows && col < self.cols);
        &self.view[row * self.row_stride + col]
    }
}

impl<'a, const ROWS: usize, const COLS: usize> From<&'a [[bool; ROWS]; COLS]>
    for StateBufferView<'a>
{
    fn from(value: &'a [[bool; ROWS]; COLS]) -> Self {
        Self::new(value.flatten(), ROWS, COLS)
    }
}

pub fn parse_plaintext(s: &str) -> StateBuffer {
    let lines = s.lines().filter(|line| !line.starts_with('!'));
    let rows = lines.clone().count();
    let cols = lines.clone().map(|line| line.len()).max().unwrap();
    let mut buf = vec![false; rows * cols];

    for (i, line) in lines.enumerate() {
        for (j, c) in line.as_bytes().iter().enumerate() {
            let state = match c {
                b'.' => false,
                b'O' => true,
                _ => panic!("unexpected char {:?}", c),
            };
            buf[i * cols + j] = state;
        }
    }

    StateBuffer::new(buf, rows, cols)
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum MacroCell {
    Leaf(MacroCellLeaf),
    Branch(CachedMacroCellBranch),
}

impl MacroCell {
    pub fn from_square(square: StateBufferView, cache: &mut Cache) -> Self {
        assert!(square.rows() == square.cols());
        assert!(square.rows().is_power_of_two() && square.rows >= 2);
        if square.rows() == 2 {
            Self::Leaf(MacroCellLeaf {
                states: [
                    [square[(0, 0)], square[(0, 1)]],
                    [square[(1, 0)], square[(1, 1)]],
                ],
            })
        } else {
            let cut = square.rows() / 2;
            let branch = MacroCellBranch {
                branches: [
                    [
                        Self::from_square(square.sub_rectangle(..cut, ..cut), cache),
                        Self::from_square(square.sub_rectangle(..cut, cut..), cache),
                    ],
                    [
                        Self::from_square(square.sub_rectangle(cut.., ..cut), cache),
                        Self::from_square(square.sub_rectangle(cut.., cut..), cache),
                    ],
                ],
            };
            let (branch, _result) = CachedMacroCellBranch::new_result(branch, cache);
            Self::Branch(branch)
        }
    }

    pub fn result(&self, cache: &Cache) -> Option<MacroCell> {
        match self {
            Self::Leaf(..) => None,
            Self::Branch(branch) => Some(branch.result(cache)),
        }
    }
}

impl From<MacroCellLeaf> for MacroCell {
    fn from(value: MacroCellLeaf) -> Self {
        Self::Leaf(value)
    }
}

impl From<CachedMacroCellBranch> for MacroCell {
    fn from(value: CachedMacroCellBranch) -> Self {
        Self::Branch(value)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MacroCellLeaf {
    pub states: [[bool; 2]; 2],
}

#[derive(Clone, PartialEq, Eq, Hash)]
pub struct MacroCellBranch {
    pub branches: [[MacroCell; 2]; 2],
}

impl MacroCellBranch {
    pub fn map_branches<T, F, G>(&self, leaf_map: F, branch_map: G) -> T
    where
        F: FnOnce([[MacroCellLeaf; 2]; 2]) -> T,
        G: FnOnce([[&CachedMacroCellBranch; 2]; 2]) -> T,
    {
        match &self.branches {
            &[[MacroCell::Leaf(top_left), MacroCell::Leaf(top_right)], [MacroCell::Leaf(bottom_left), MacroCell::Leaf(bottom_right)]] => {
                leaf_map([[top_left, top_right], [bottom_left, bottom_right]])
            }
            [[MacroCell::Branch(top_left), MacroCell::Branch(top_right)], [MacroCell::Branch(bottom_left), MacroCell::Branch(bottom_right)]] => {
                branch_map([[top_left, top_right], [bottom_left, bottom_right]])
            }
            _ => {
                unreachable!("branch has mix of branches and leaves as children");
            }
        }
    }

    pub fn compute_result(&self, cache: &mut Cache) -> MacroCell {
        self.map_branches(
            |leaves: [[MacroCellLeaf; 2]; 2]| -> MacroCell {
                let mut states = [[false; 4]; 4];
                for i in 0..4 {
                    for j in 0..4 {
                        states[i][j] = leaves[i >> 1][j >> 1].states[i & 1][j & 1];
                    }
                }

                let mut result = [[false; 2]; 2];
                const NEIGHBORS: [(usize, usize); 8] = [
                    (0, 0),
                    (1, 0),
                    (0, 1),
                    (2, 0),
                    (0, 2),
                    (2, 1),
                    (1, 2),
                    (2, 2),
                ];
                for i in 0..2 {
                    for j in 0..2 {
                        let alive_neighbors = NEIGHBORS
                            .into_iter()
                            .filter(|(di, dj)| states[i + di][j + dj])
                            .count();
                        let self_state = states[i + 1][j + 1];
                        let next_state = match (self_state, alive_neighbors) {
                            (false, 3) | (true, 2..=3) => true,
                            _ => false,
                        };
                        result[i][j] = next_state;
                    }
                }
                MacroCell::Leaf(MacroCellLeaf { states: result })
            },
            |branches: [[&CachedMacroCellBranch; 2]; 2]| -> MacroCell {
                fn horizontal_shift_result(
                    left: &CachedMacroCellBranch,
                    right: &CachedMacroCellBranch,
                    cache: &mut Cache,
                ) -> MacroCell {
                    let quadrants = [
                        [left.branches[0][1].clone(), right.branches[0][0].clone()],
                        [left.branches[1][1].clone(), right.branches[1][0].clone()],
                    ];
                    let (_, result) = CachedMacroCellBranch::new_result(
                        MacroCellBranch {
                            branches: quadrants,
                        },
                        cache,
                    );
                    result
                }
                fn vertical_shift_result(
                    top: &CachedMacroCellBranch,
                    bottom: &CachedMacroCellBranch,
                    cache: &mut Cache,
                ) -> MacroCell {
                    let quadrants = [
                        [top.branches[1][0].clone(), top.branches[1][1].clone()],
                        [bottom.branches[0][0].clone(), bottom.branches[0][1].clone()],
                    ];
                    let (_, result) = CachedMacroCellBranch::new_result(
                        MacroCellBranch {
                            branches: quadrants,
                        },
                        cache,
                    );
                    result
                }
                fn corner_shift_result(
                    quadrants: [[&CachedMacroCellBranch; 2]; 2],
                    cache: &mut Cache,
                ) -> MacroCell {
                    let corner_quadrants = [
                        [
                            quadrants[0][0].branches[1][1].clone(),
                            quadrants[0][1].branches[1][0].clone(),
                        ],
                        [
                            quadrants[1][0].branches[0][1].clone(),
                            quadrants[1][1].branches[0][0].clone(),
                        ],
                    ];
                    let (_, result) = CachedMacroCellBranch::new_result(
                        MacroCellBranch {
                            branches: corner_quadrants,
                        },
                        cache,
                    );
                    result
                }

                let shifted_results: [[MacroCell; 3]; 3] = [
                    [
                        branches[0][0].result(cache),
                        horizontal_shift_result(&branches[0][0], &branches[0][1], cache),
                        branches[0][1].result(cache),
                    ],
                    [
                        vertical_shift_result(&branches[0][0], &branches[1][0], cache),
                        corner_shift_result(branches, cache),
                        vertical_shift_result(&branches[0][1], &branches[1][1], cache),
                    ],
                    [
                        branches[1][0].result(cache),
                        horizontal_shift_result(&branches[1][0], &branches[1][1], cache),
                        branches[1][1].result(cache),
                    ],
                ];

                let mut get_overlap_result = |i: usize, j: usize| -> MacroCell {
                    let quadrants = [
                        [
                            shifted_results[i][j].clone(),
                            shifted_results[i][j + 1].clone(),
                        ],
                        [
                            shifted_results[i + 1][j].clone(),
                            shifted_results[i + 1][j + 1].clone(),
                        ],
                    ];
                    let (_, result) = CachedMacroCellBranch::new_result(
                        MacroCellBranch {
                            branches: quadrants,
                        },
                        cache,
                    );
                    result
                };
                let overlapping_quadrants_results: [[MacroCell; 2]; 2] = [
                    [get_overlap_result(0, 0), get_overlap_result(0, 1)],
                    [get_overlap_result(1, 0), get_overlap_result(1, 1)],
                ];

                let (branch, _) = CachedMacroCellBranch::new_result(
                    MacroCellBranch {
                        branches: overlapping_quadrants_results,
                    },
                    cache,
                );
                MacroCell::Branch(branch)
            },
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_result(world_grid: &str, expected_grid: &str) {
        let mut cache = Cache::new();

        let world_buf = parse_plaintext(world_grid);
        let expected_buf = parse_plaintext(expected_grid);

        let world = MacroCell::from_square(world_buf.view(), &mut cache);
        let result = world.result(&cache).unwrap();

        let expected = MacroCell::from_square(expected_buf.view(), &mut cache);

        assert_eq!(result, expected);
    }

    #[test]
    fn block_pattern() {
        assert_result(
            "\
                ........\n\
                ........\n\
                ........\n\
                ...OO...\n\
                ...OO...\n\
                ........\n\
                ........\n\
                ........\n\
            ",
            "\
                ....\n\
                .OO.\n\
                .OO.\n\
                ....\n\
            ",
        )
    }

    #[test]
    fn beehive_pattern() {
        assert_result(
            "\
                ........\n\
                ........\n\
                ...OO...\n\
                ..O..O..\n\
                ...OO...\n\
                ........\n\
                ........\n\
                ........\n\
            ",
            "\
                .OO.\n\
                O..O\n\
                .OO.\n\
                ....\n\
            ",
        )
    }

    #[test]
    fn loaf_pattern() {
        assert_result(
            "\
                ........\n\
                ........\n\
                ...OO...\n\
                ..O..O..\n\
                ...O.O..\n\
                ....O...\n\
                ........\n\
                ........\n\
            ",
            "\
                .OO.\n\
                O..O\n\
                .O.O\n\
                ..O.\n\
            ",
        )
    }

    #[test]
    fn pond_pattern() {
        assert_result(
            "\
                ........\n\
                ........\n\
                ...OO...\n\
                ..O..O..\n\
                ..O..O..\n\
                ...OO...\n\
                ........\n\
                ........\n\
            ",
            "\
                .OO.\n\
                O..O\n\
                O..O\n\
                .OO.\n\
            ",
        )
    }

    #[test]
    fn ship_tie_pattern() {
        assert_result(
            "\
                ................\n\
                ................\n\
                ................\n\
                ................\n\
                ................\n\
                .........OO.....\n\
                ........O.O.....\n\
                ........OO......\n\
                ......OO........\n\
                .....O.O........\n\
                .....OO.........\n\
                ................\n\
                ................\n\
                ................\n\
                ................\n\
                ................\n\
            ",
            "\
                ........\n\
                .....OO.\n\
                ....O.O.\n\
                ....OO..\n\
                ..OO....\n\
                .O.O....\n\
                .OO.....\n\
                ........\n\
            ",
        )
    }
}