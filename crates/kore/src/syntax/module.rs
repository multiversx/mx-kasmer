use super::{Attr, Id, Sentence};

/// `module NAME sentences endmodule [...]`
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Module {
    pub name: Id,
    pub sentences: Vec<Sentence>,
    pub attrs: Vec<Attr>,
}
