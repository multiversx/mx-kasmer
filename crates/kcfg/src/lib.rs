//! Proof graphs (K control flow graphs) of symbolic execution on the K JSON-RPC servers, in
//! KORE, as pyk's `pyk.kcfg`:
//!
//! - [`cterm`]: constrained terms, a configuration and its path condition;
//! - [`graph`]: the graph, with edges, covers, splits and non-deterministic branches;
//! - [`extend`]: from an `execute` result to the extension of a node, as pyk reads it;
//! - [`explore`]: extending a node and checking implication on the server;
//! - [`rules`]: names of rules for the edges, from the definition;
//! - [`store`]: the graph on disk, written as the proof goes, so that it can be resumed.
//!
//! Checked against a proof recorded from pyk (`scripts/rpc-corpus.sh`, `tests/conformance.rs`).

pub mod cterm;
pub mod explore;
pub mod extend;
pub mod graph;
pub mod rules;
pub mod store;

pub use cterm::{CSubst, CTerm};
pub use extend::{Extension, apply, classify};
pub use graph::{Kcfg, Node, NodeId, Successor};
pub use rules::RuleLabels;
pub use store::Store;
