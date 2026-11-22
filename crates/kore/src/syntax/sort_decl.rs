use super::{Attr, Id};

/// `sort SortInt{} [...]`, or `hooked-sort`.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct SortDecl {
    pub name: Id,
    pub vars: Vec<Id>,
    pub hooked: bool,
    pub attrs: Vec<Attr>,
}
