//! The KORE term model: sorts, patterns, sentences, modules and definitions.
//!
//! Follows the model of pyk (`pyk.kore.syntax`), with one difference in shape: all
//! matching-logic connectives share one variant, [`Ml`], since they differ only in the number of
//! sorts, the bound variable and the number of arguments. [`Ml::new`] checks those counts.
//!
//! Terms can be deep: K lists and maps are nested applications, one level per element.
//! Dropping a pattern does not recurse (see [`Patterns`]); the derived `Clone`, `PartialEq` and
//! `Debug` do.

use std::fmt;
use std::ops::{Deref, DerefMut};
use std::sync::Arc;

/// An identifier: sort, symbol, variable or module name. Cheap to clone.
pub type Id = Arc<str>;

#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub enum Sort {
    /// A sort variable, such as `S` in `\top{S}()` inside a parametric axiom.
    Var(Id),
    /// A sort constructor applied to sorts: `SortInt{}`, `SortMap{}`, `SortList{SortInt{}}`.
    App(Id, Vec<Sort>),
}

impl Sort {
    /// A sort without parameters, such as `SortInt{}`.
    pub fn app0(name: &str) -> Sort {
        Sort::App(name.into(), Vec::new())
    }
}

/// A variable with its sort. Element variables are written `X : S`, set variables `@X : S`;
/// the `@` is part of the name.
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

#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub enum Pattern {
    /// Element variable.
    EVar(Var),
    /// Set variable (name starts with `@`).
    SVar(Var),
    /// String literal. The value is the decoded string; quoting is done by the printers.
    String(String),
    /// Domain value: `\dv{SortInt{}}("42")`.
    Dv { sort: Sort, value: String },
    /// Symbol application: `Lbl'Unds'+Int'Unds'{}(X : SortInt{}, \dv{SortInt{}}("1"))`.
    App(App),
    /// A matching-logic connective: `\and`, `\equals`, `\exists`, ...
    Ml(Ml),
    /// `\left-assoc{}(f{}(a, b, c))` or `\right-assoc{}(...)`: `f` applied to its arguments,
    /// associating to the left or to the right.
    Assoc(Assoc),
}

#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct App {
    pub symbol: Id,
    pub sorts: Vec<Sort>,
    pub args: Patterns,
}

impl App {
    pub fn new(symbol: impl Into<Id>, sorts: Vec<Sort>, args: Vec<Pattern>) -> App {
        App {
            symbol: symbol.into(),
            sorts,
            args: args.into(),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum AssocDir {
    Left,
    Right,
}

#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct Assoc {
    pub dir: AssocDir,
    pub app: App,
}

/// A matching-logic connective. The field contents depend on the connective, see [`Conn`].
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct Ml {
    pub conn: Conn,
    pub sorts: Vec<Sort>,
    pub var: Option<Var>,
    pub args: Patterns,
}

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

impl Ml {
    /// Checks the counts of sorts and arguments and the bound variable against the connective.
    pub fn new(conn: Conn, sorts: Vec<Sort>, var: Option<Var>, args: Vec<Pattern>) -> Result<Ml, String> {
        let shape = conn.shape();
        let name = conn.symbol();
        if sorts.len() != shape.sorts {
            return Err(format!("{name} takes {} sorts, got {}", shape.sorts, sorts.len()));
        }
        match (shape.binder, &var) {
            (Binder::None, None) => {}
            (Binder::Element, Some(v)) if !v.is_set_var() => {}
            (Binder::Set, Some(v)) if v.is_set_var() => {}
            (Binder::None, Some(_)) => return Err(format!("{name} binds no variable")),
            (Binder::Element, _) => return Err(format!("{name} binds an element variable")),
            (Binder::Set, _) => return Err(format!("{name} binds a set variable")),
        }
        if let Arity::Exactly(n) = shape.args
            && args.len() != n
        {
            return Err(format!("{name} takes {n} arguments, got {}", args.len()));
        }
        Ok(Ml {
            conn,
            sorts,
            var,
            args: args.into(),
        })
    }
}

/// The arguments of an application or connective.
///
/// A `Vec<Pattern>` whose drop does not recurse: dropping a term nested a million levels deep
/// would otherwise overflow the stack. Dereferences to the vector.
#[derive(Clone, PartialEq, Eq, Hash, Default)]
pub struct Patterns(Vec<Pattern>);

impl Patterns {
    pub fn into_vec(mut self) -> Vec<Pattern> {
        std::mem::take(&mut self.0)
    }
}

impl Deref for Patterns {
    type Target = Vec<Pattern>;
    fn deref(&self) -> &Vec<Pattern> {
        &self.0
    }
}

impl DerefMut for Patterns {
    fn deref_mut(&mut self) -> &mut Vec<Pattern> {
        &mut self.0
    }
}

impl From<Vec<Pattern>> for Patterns {
    fn from(v: Vec<Pattern>) -> Patterns {
        Patterns(v)
    }
}

impl fmt::Debug for Patterns {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl Drop for Patterns {
    fn drop(&mut self) {
        let mut stack = std::mem::take(&mut self.0);
        while let Some(mut p) = stack.pop() {
            if let Some(args) = p.args_mut() {
                stack.append(args);
            }
            // `p` is dropped here with no arguments left.
        }
    }
}

impl Pattern {
    /// The direct subpatterns.
    pub fn args(&self) -> &[Pattern] {
        match self {
            Pattern::App(a) => &a.args,
            Pattern::Ml(m) => &m.args,
            Pattern::Assoc(a) => &a.app.args,
            Pattern::EVar(_) | Pattern::SVar(_) | Pattern::String(_) | Pattern::Dv { .. } => &[],
        }
    }

    fn args_mut(&mut self) -> Option<&mut Vec<Pattern>> {
        match self {
            Pattern::App(a) => Some(&mut a.args.0),
            Pattern::Ml(m) => Some(&mut m.args.0),
            Pattern::Assoc(a) => Some(&mut a.app.args.0),
            Pattern::EVar(_) | Pattern::SVar(_) | Pattern::String(_) | Pattern::Dv { .. } => None,
        }
    }

    pub fn app(symbol: impl Into<Id>, sorts: Vec<Sort>, args: Vec<Pattern>) -> Pattern {
        Pattern::App(App::new(symbol, sorts, args))
    }

    pub fn dv(sort: Sort, value: impl Into<String>) -> Pattern {
        Pattern::Dv {
            sort,
            value: value.into(),
        }
    }

    pub fn evar(name: impl Into<Id>, sort: Sort) -> Pattern {
        Pattern::EVar(Var::new(name, sort))
    }

    /// Calls `f` on every subpattern, parents before children, left to right. Does not recurse.
    pub fn visit(&self, mut f: impl FnMut(&Pattern)) {
        let mut stack = vec![self];
        while let Some(p) = stack.pop() {
            f(p);
            stack.extend(p.args().iter().rev());
        }
    }

    /// The nesting depth: 1 for a leaf. Does not recurse.
    pub fn depth(&self) -> usize {
        let mut max = 0;
        let mut stack = vec![(self, 1)];
        while let Some((p, d)) = stack.pop() {
            max = max.max(d);
            stack.extend(p.args().iter().map(|a| (a, d + 1)));
        }
        max
    }
}

/// An attribute: a symbol applied to patterns, usually strings, such as `hook{}("INT.add")`.
pub type Attr = App;

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Sentence {
    Import { module: Id, attrs: Vec<Attr> },
    Sort(SortDecl),
    Symbol(SymbolDecl),
    Alias(AliasDecl),
    Axiom(Axiom),
    Claim(Axiom),
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct SortDecl {
    pub name: Id,
    pub vars: Vec<Id>,
    pub hooked: bool,
    pub attrs: Vec<Attr>,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct SymbolDecl {
    pub name: Id,
    /// Sort variables of a parametric symbol: `S` in `symbol inj{From, To}(From) : To`.
    pub vars: Vec<Id>,
    pub params: Vec<Sort>,
    pub sort: Sort,
    pub hooked: bool,
    pub attrs: Vec<Attr>,
}

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

/// An axiom or a claim.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Axiom {
    pub vars: Vec<Id>,
    pub pattern: Pattern,
    pub attrs: Vec<Attr>,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Module {
    pub name: Id,
    pub sentences: Vec<Sentence>,
    pub attrs: Vec<Attr>,
}

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
