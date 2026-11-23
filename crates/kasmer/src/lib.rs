//! Parametric tests of MultiversX smart contracts on the K semantics of the VM.
//!
//! A test project is a test contract whose `test_*` endpoints take arguments, plus the contracts it
//! tests (see [`project`]). The test contract is deployed into a configuration of the semantics
//! (see [`setup`]); each test endpoint is then called from that configuration.
//!
//! - Concrete: [`fuzz`] calls each endpoint with generated arguments on the LLVM interpreter of
//!   the semantics and fails on a non-zero exit code. It replaces `kasmer fuzz` of the Python
//!   tooling (`kmultiversx.kasmer`), and builds the same terms.
//! - Symbolic: [`claim`] states that an endpoint passes for all arguments, and [`prove`] proves it
//!   on `kore-rpc-booster`, with cut points, [`abstraction`] of large cells and [`wasm_init`] of
//!   modules on the LLVM interpreter. It replaces `kasmer claims` and `kasmer verify`
//!   (`kmxwasm`), builds the same claims and the same proof graphs.
//!
//! The K side is a compiled LLVM definition ([`LlvmDefinition`]): its `definition.kore`, for the
//! symbols, and its `interpreter` executable, run as a separate process ([`interpreter`]).

pub mod abstraction;
pub mod cells;
pub mod claim;
pub mod definition;
mod error;
pub mod fuzz;
pub mod interpreter;
pub mod mandos;
pub mod project;
pub mod prove;
pub mod setup;
pub mod steps;
pub mod wasm_init;

pub use definition::LlvmDefinition;
pub use error::Error;

/// The KORE symbol of a K label.
pub(crate) fn symbol_of(label: &str) -> String {
    kore::symbols::label_to_symbol(label).expect("labels in the code are ASCII")
}
