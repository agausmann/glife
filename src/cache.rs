use std::{
    collections::HashMap,
    fmt,
    hash::{Hash, Hasher},
    ops::Deref,
    rc::Rc,
};

use crate::macro_cell::{MacroCell, MacroCellBranch};

#[derive(Clone)]
pub struct CachedMacroCellBranch(Rc<MacroCellBranch>);

impl CachedMacroCellBranch {
    pub fn new_result(branch: MacroCellBranch, cache: &mut Cache) -> (Self, MacroCell) {
        if let Some((key, result)) = cache.result.get_key_value(&branch) {
            (Self(key.clone()), result.clone())
        } else {
            let result = branch.compute_result(cache);
            let rc = Rc::new(branch);
            cache.result.insert(Rc::clone(&rc), result.clone());
            (Self(rc), result)
        }
    }

    pub fn result(&self, cache: &Cache) -> MacroCell {
        cache.result[&self.0].clone()
    }
}

impl fmt::Debug for CachedMacroCellBranch {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_tuple("CachedMacroCellBranch")
            .field(&Rc::as_ptr(&self.0))
            .finish()
    }
}

impl PartialEq for CachedMacroCellBranch {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}

impl Eq for CachedMacroCellBranch {}

impl Hash for CachedMacroCellBranch {
    fn hash<H: Hasher>(&self, state: &mut H) {
        Rc::as_ptr(&self.0).hash(state)
    }
}

impl Deref for CachedMacroCellBranch {
    type Target = MacroCellBranch;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

pub struct Cache {
    result: HashMap<Rc<MacroCellBranch>, MacroCell>,
}

impl Cache {
    pub fn new() -> Self {
        Self {
            result: HashMap::new(),
        }
    }
}
