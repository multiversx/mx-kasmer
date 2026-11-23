use super::{Attr, Id, Pattern};

/// An axiom or a claim: `axiom{vars} pattern [...]`.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Axiom {
    pub vars: Vec<Id>,
    pub pattern: Pattern,
    pub attrs: Vec<Attr>,
}
