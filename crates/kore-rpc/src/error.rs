use std::fmt;

use serde::Deserialize;
use serde_json::Value;

#[derive(Debug)]
pub enum Error {
    Io(std::io::Error),
    /// A message that is not the JSON it should be.
    Json(serde_json::Error),
    /// The server answered with an error.
    Rpc(RpcError),
    /// The connection or the server does not behave as the protocol says: closed connection,
    /// mismatched id, server not starting.
    Protocol(String),
}

/// The `error` of a JSON-RPC response.
#[derive(Clone, Debug, Deserialize)]
pub struct RpcError {
    pub code: i64,
    pub message: String,
    #[serde(default)]
    pub data: Option<Value>,
}

/// The error codes of the K servers, as `pyk.kore.rpc.KoreClient` reads them.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ErrorKind {
    /// 1: a pattern that does not parse.
    Parse,
    /// 2: a pattern that is not well formed for the definition.
    Pattern,
    /// 3
    UnknownModule,
    /// 4
    Implication,
    /// 5: the SMT solver failed (timeout, unknown).
    SmtSolver,
    /// 8
    InvalidModule,
    /// 9
    DuplicateModule,
    Other,
}

impl RpcError {
    pub fn kind(&self) -> ErrorKind {
        match self.code {
            1 => ErrorKind::Parse,
            2 => ErrorKind::Pattern,
            3 => ErrorKind::UnknownModule,
            4 => ErrorKind::Implication,
            5 => ErrorKind::SmtSolver,
            8 => ErrorKind::InvalidModule,
            9 => ErrorKind::DuplicateModule,
            _ => ErrorKind::Other,
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Io(e) => write!(f, "connection: {e}"),
            Error::Json(e) => write!(f, "message: {e}"),
            Error::Rpc(e) => {
                write!(f, "server error {}: {}", e.code, e.message)?;
                if let Some(data) = &e.data {
                    let data = data.to_string();
                    let end = data.floor_char_boundary(1000);
                    write!(f, ": {}", &data[..end])?;
                    if end < data.len() {
                        f.write_str("...")?;
                    }
                }
                Ok(())
            }
            Error::Protocol(m) => f.write_str(m),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Error::Io(e) => Some(e),
            Error::Json(e) => Some(e),
            _ => None,
        }
    }
}

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Error {
        Error::Io(e)
    }
}

impl From<serde_json::Error> for Error {
    fn from(e: serde_json::Error) -> Error {
        Error::Json(e)
    }
}
