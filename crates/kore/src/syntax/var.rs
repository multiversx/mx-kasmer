use super::{Id, Sort};

/// A variable with its sort. Element variables are written `X : S`, set variables `@X : S`;
/// the `@` is part of the name.
///
/// In KORE JSON a variable is a pattern (`EVar`, `SVar`, see [`Pattern`](super::Pattern)) or the
/// `var` and `varSort` fields of a binder (see [`Ml`](super::Ml)); it has no format of its own.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct Var {
    pub name: Id,
    pub sort: Sort,
}

impl Var {
    pub fn new(name: impl Into<Id>, sort: Sort) -> Var {
        Var {
            name: name.into(),
            sort,
        }
    }

    pub fn is_set_var(&self) -> bool {
        self.name.starts_with('@')
    }
}
