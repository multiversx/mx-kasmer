//! Parametric tests of MultiversX smart contracts on the K semantics of the VM.
//!
//! A test project is a test contract whose `test_*` endpoints take arguments, plus the contracts it
//! tests (see [`project`]). The test contract is deployed into a configuration of the semantics
//! (see [`setup`]); each test endpoint is then called from that configuration.
//!
//! So far the concrete path: [`fuzz`] calls each endpoint with generated arguments on the LLVM
//! interpreter of the semantics and fails on a non-zero exit code. It replaces `kasmer fuzz` of the
//! Python tooling (`kmultiversx.kasmer`), and builds the same terms.
//!
//! The K side is a compiled LLVM definition ([`LlvmDefinition`]): its `definition.kore`, for the
//! symbols, and its `interpreter` executable, run as a separate process ([`interpreter`]).

pub mod cells;
pub mod definition;
mod error;
pub mod fuzz;
pub mod interpreter;
pub mod mandos;
pub mod project;
pub mod setup;
pub mod steps;

pub use definition::LlvmDefinition;
pub use error::Error;
