use super::{Attr, Id, Sort};

/// `symbol name{vars}(params) : sort [...]`, or `hooked-symbol`.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct SymbolDecl {
    pub name: Id,
    /// Sort variables of a parametric symbol: `From` and `To` in `symbol inj{From, To}(From) : To`.
    pub vars: Vec<Id>,
    pub params: Vec<Sort>,
    pub sort: Sort,
    pub hooked: bool,
    pub attrs: Vec<Attr>,
}
