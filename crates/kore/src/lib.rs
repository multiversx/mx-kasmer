//! KORE, the intermediate language of the K framework: term model, text format and JSON format.
//!
//! - [`syntax`]: sorts, patterns, sentences, modules, definitions.
//! - [`text`]: KORE text, as written by the K compiler (`definition.kore`) and read and written
//!   by the LLVM interpreter. Parser and printer.
//! - [`json`]: KORE JSON, as spoken by the K JSON-RPC servers.
//!
//! The printers produce the same text as pyk, byte for byte; `tests/conformance.rs` checks this
//! against reference data written by pyk (`scripts/kore-corpus.sh`).
//!
//! Terms of any depth are handled: the text parser and printer and dropping do not recurse; JSON
//! goes through `serde_json` with `serde_stacker`, which grows the stack as needed.

pub mod json;
pub mod quote;
pub mod syntax;
pub mod text;

pub use syntax::*;
