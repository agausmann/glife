use std::ops::{Bound, Index, Range, RangeBounds};

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
