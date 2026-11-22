//! Client of the K JSON-RPC servers, `kore-rpc` (Haskell backend) and `kore-rpc-booster` (the
//! booster in front of it, which evaluates concrete subterms with the LLVM backend).
//!
//! - [`protocol`]: the messages: `execute`, `implies`, `simplify`, `get-model`, `add-module`.
//!   Patterns are [`kore::Pattern`]s, in KORE JSON on the wire.
//! - [`Client`]: one connection; JSON-RPC 2.0, one message per line. Can write a transcript of
//!   every exchange.
//! - [`Server`]: starts a server from a server directory and connects to it, also when it runs in
//!   a container (see [`server`]).
//!
//! Checked against pyk's `pyk.kore.rpc` on a recorded proof (`scripts/rpc-corpus.sh`,
//! `tests/conformance.rs`).

pub mod client;
pub mod error;
pub mod protocol;
pub mod server;

pub use client::Client;
pub use error::{Error, ErrorKind, RpcError};
pub use protocol::Kore;
pub use server::Server;
