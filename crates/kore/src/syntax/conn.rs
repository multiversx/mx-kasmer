/// The matching-logic connectives other than `\dv`, `\left-assoc` and `\right-assoc`.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Conn {
    Top,
    Bottom,
    Not,
    And,
    Or,
    Implies,
    Iff,
    Exists,
    Forall,
    Mu,
    Nu,
    Ceil,
    Floor,
    Equals,
    In,
    Next,
    Rewrites,
}

/// What a connective takes: number of sorts, kind of bound variable, number of arguments.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Shape {
    pub sorts: usize,
    pub binder: Binder,
    pub args: Arity,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Binder {
    None,
    /// `\exists`, `\forall`
    Element,
    /// `\mu`, `\nu`
    Set,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Arity {
    Exactly(usize),
    /// `\and`, `\or`
    Any,
}

impl Conn {
    pub const ALL: [Conn; 17] = [
        Conn::Top,
        Conn::Bottom,
        Conn::Not,
        Conn::And,
        Conn::Or,
        Conn::Implies,
        Conn::Iff,
        Conn::Exists,
        Conn::Forall,
        Conn::Mu,
        Conn::Nu,
        Conn::Ceil,
        Conn::Floor,
        Conn::Equals,
        Conn::In,
        Conn::Next,
        Conn::Rewrites,
    ];

    /// The KORE text symbol, with its backslash.
    pub fn symbol(self) -> &'static str {
        match self {
            Conn::Top => r"\top",
            Conn::Bottom => r"\bottom",
            Conn::Not => r"\not",
            Conn::And => r"\and",
            Conn::Or => r"\or",
            Conn::Implies => r"\implies",
            Conn::Iff => r"\iff",
            Conn::Exists => r"\exists",
            Conn::Forall => r"\forall",
            Conn::Mu => r"\mu",
            Conn::Nu => r"\nu",
            Conn::Ceil => r"\ceil",
            Conn::Floor => r"\floor",
            Conn::Equals => r"\equals",
            Conn::In => r"\in",
            Conn::Next => r"\next",
            Conn::Rewrites => r"\rewrites",
        }
    }

    pub fn from_symbol(symbol: &str) -> Option<Conn> {
        Conn::ALL.into_iter().find(|c| c.symbol() == symbol)
    }

    /// The `tag` of the KORE JSON format.
    pub fn json_tag(self) -> &'static str {
        match self {
            Conn::Top => "Top",
            Conn::Bottom => "Bottom",
            Conn::Not => "Not",
            Conn::And => "And",
            Conn::Or => "Or",
            Conn::Implies => "Implies",
            Conn::Iff => "Iff",
            Conn::Exists => "Exists",
            Conn::Forall => "Forall",
            Conn::Mu => "Mu",
            Conn::Nu => "Nu",
            Conn::Ceil => "Ceil",
            Conn::Floor => "Floor",
            Conn::Equals => "Equals",
            Conn::In => "In",
            Conn::Next => "Next",
            Conn::Rewrites => "Rewrites",
        }
    }

    pub fn from_json_tag(tag: &str) -> Option<Conn> {
        Conn::ALL.into_iter().find(|c| c.json_tag() == tag)
    }

    pub fn shape(self) -> Shape {
        use Arity::*;
        let (sorts, binder, args) = match self {
            Conn::Top | Conn::Bottom => (1, Binder::None, Exactly(0)),
            Conn::Not | Conn::Next => (1, Binder::None, Exactly(1)),
            Conn::And | Conn::Or => (1, Binder::None, Any),
            Conn::Implies | Conn::Iff | Conn::Rewrites => (1, Binder::None, Exactly(2)),
            Conn::Exists | Conn::Forall => (1, Binder::Element, Exactly(1)),
            Conn::Mu | Conn::Nu => (0, Binder::Set, Exactly(1)),
            Conn::Ceil | Conn::Floor => (2, Binder::None, Exactly(1)),
            Conn::Equals | Conn::In => (2, Binder::None, Exactly(2)),
        };
        Shape { sorts, binder, args }
    }
}
