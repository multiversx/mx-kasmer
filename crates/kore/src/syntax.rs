//! The KORE term model: sorts, patterns, sentences, modules and definitions.
//!
//! Follows the model of pyk (`pyk.kore.syntax`), with one difference in shape: all
//! matching-logic connectives share one variant, [`Ml`], since they differ only in the number of
//! sorts, the bound variable and the number of arguments. [`Ml::new`] checks those counts.
//!
//! Terms can be deep: K lists and maps are nested applications, one level per element.
//! Dropping a pattern does not recurse (see [`Patterns`]); the derived `Clone`, `PartialEq` and
//! `Debug` do.
//!
//! [`Sort`] and [`Pattern`] implement `Serialize` and `Deserialize` in the KORE JSON format, each
//! next to its type. Fields are written in alphabetical order, as pyk writes them
//! (`sort_keys=True`). When reading, the type of each field follows from its name alone, so
//! objects are read in one pass whatever the order of their fields (pyk puts `tag` last). See
//! [`crate::json`] for reading and writing strings.

mod alias_decl;
mod app;
mod assoc;
mod axiom;
mod conn;
mod definition;
pub(crate) mod fields;
mod ml;
mod module;
mod pattern;
mod patterns;
mod sentence;
mod sort;
mod sort_decl;
mod symbol_decl;
mod var;

use std::sync::Arc;

pub use alias_decl::AliasDecl;
pub use app::App;
pub use assoc::{Assoc, AssocDir};
pub use axiom::Axiom;
pub use conn::{Arity, Binder, Conn, Shape};
pub use definition::Definition;
pub use ml::Ml;
pub use module::Module;
pub use pattern::Pattern;
pub use patterns::Patterns;
pub use sentence::Sentence;
pub use sort::Sort;
pub use sort_decl::SortDecl;
pub use symbol_decl::SymbolDecl;
pub use var::Var;

/// An identifier: sort, symbol, variable or module name. Cheap to clone.
pub type Id = Arc<str>;

/// An attribute: a symbol applied to patterns, usually strings, such as `hook{}("INT.add")`.
pub type Attr = App;
