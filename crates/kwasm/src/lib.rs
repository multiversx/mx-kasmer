//! Wasm modules as terms of the K semantics of WebAssembly (wasm-semantics).
//!
//! [`module_term`] reads a Wasm binary and builds the KORE term of sort `ModuleDecl` that the
//! semantics loads. It replaces pykwasm's `wasm2kast` followed by pyk's `kast_to_kore`, and
//! produces the same term, byte for byte; `tests/conformance.rs` checks this on every contract
//! in `contracts/` (reference data from `scripts/kore-corpus.sh`).
//!
//! The binary is read with `wasmparser`, which is not given a validator: as with pykwasm, the
//! module is converted as it is, and only what the semantics cannot represent is rejected.
//!
//! Deliberate difference from pykwasm: `i64.extend8_s`, `i64.extend16_s` and `i64.extend32_s`
//! get the type `i64`; pykwasm gives them `i32`.

mod ast;
mod instructions;
mod module;

use std::fmt;

use kore::symbols::{BuildError, Symbols, Term};

/// Converts a Wasm binary into the term of its module, of sort `ModuleDecl`.
///
/// `symbols` are those of a definition that includes the Wasm semantics. `filename` goes into the
/// module metadata (`moduleMeta`), where the MultiversX semantics compares it with the code of
/// accounts.
pub fn module_term(wasm: &[u8], filename: Option<&str>, symbols: &Symbols) -> Result<Term, Error> {
    module::module(&ast::Ast::new(symbols), wasm, filename)
}

#[derive(Debug)]
pub enum Error {
    /// Not a well-formed Wasm binary.
    Parse(wasmparser::BinaryReaderError),
    /// A feature the semantics does not support.
    Unsupported { offset: usize, what: String },
    /// A label is missing from the definition: the semantics and this crate disagree.
    Build(BuildError),
}

impl Error {
    pub(crate) fn unsupported(offset: impl TryInto<usize>, what: impl Into<String>) -> Error {
        Error::Unsupported {
            offset: offset.try_into().unwrap_or(usize::MAX),
            what: what.into(),
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Parse(e) => write!(f, "invalid Wasm: {e}"),
            Error::Unsupported { offset, what } => {
                write!(f, "at byte {offset:#x}: not supported: {what}")
            }
            Error::Build(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for Error {}

impl From<wasmparser::BinaryReaderError> for Error {
    fn from(e: wasmparser::BinaryReaderError) -> Error {
        Error::Parse(e)
    }
}

impl From<BuildError> for Error {
    fn from(e: BuildError) -> Error {
        Error::Build(e)
    }
}
