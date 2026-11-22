use super::{AliasDecl, Attr, Axiom, Id, SortDecl, SymbolDecl};

/// A sentence of a module.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Sentence {
    Import { module: Id, attrs: Vec<Attr> },
    Sort(SortDecl),
    Symbol(SymbolDecl),
    Alias(AliasDecl),
    Axiom(Axiom),
    Claim(Axiom),
}
