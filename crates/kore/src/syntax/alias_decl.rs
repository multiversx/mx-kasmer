use super::{App, Attr, Id, Pattern, Sort};

/// `alias name{vars}(params) : sort where left := right [...]`
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct AliasDecl {
    pub name: Id,
    pub vars: Vec<Id>,
    pub params: Vec<Sort>,
    pub sort: Sort,
    pub left: App,
    pub right: Pattern,
    pub attrs: Vec<Attr>,
}
