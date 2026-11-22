use super::{Attr, Axiom, Module, Sentence, SymbolDecl};

/// A whole definition, as in `definition.kore`: attributes, then modules.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Definition {
    pub attrs: Vec<Attr>,
    pub modules: Vec<Module>,
}

impl Definition {
    pub fn axioms(&self) -> impl Iterator<Item = &Axiom> {
        self.modules.iter().flat_map(|m| &m.sentences).filter_map(|s| match s {
            Sentence::Axiom(a) => Some(a),
            _ => None,
        })
    }

    pub fn symbol_decls(&self) -> impl Iterator<Item = &SymbolDecl> {
        self.modules.iter().flat_map(|m| &m.sentences).filter_map(|s| match s {
            Sentence::Symbol(d) => Some(d),
            _ => None,
        })
    }
}
