//! One connection to a K JSON-RPC server.
//!
//! JSON-RPC 2.0 over a byte stream, one message per line, as pyk's single-socket transport: the
//! request is written followed by a newline, the response is the next line. The stream is a TCP
//! connection, or the standard input and output of a process that forwards them (see
//! [`crate::server`]). Requests are answered one at a time, in order.

use std::io::{self, BufRead, BufReader, BufWriter, Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::process::Child;
use std::time::{Duration, Instant};

use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::Value;

use crate::error::{Error, RpcError};
use crate::protocol::*;

pub struct Client {
    reader: Box<dyn BufRead + Send>,
    writer: Box<dyn Write + Send>,
    /// The process that forwards the connection, if any; stopped with the client.
    forwarder: Option<Child>,
    label: String,
    next_id: u64,
    transcript: Option<Box<dyn Write + Send>>,
}

#[derive(Serialize)]
struct Request<'a, P> {
    jsonrpc: &'static str,
    id: &'a str,
    method: &'a str,
    params: &'a P,
}

impl Client {
    /// Connects over TCP, retrying while the connection is refused, up to `timeout`.
    pub fn connect_tcp(addr: impl ToSocketAddrs, timeout: Duration) -> Result<Client, Error> {
        let start = Instant::now();
        let addrs: Vec<_> = addr.to_socket_addrs()?.collect();
        loop {
            let mut last = None;
            for a in &addrs {
                match TcpStream::connect(a) {
                    Ok(stream) => {
                        stream.set_nodelay(true)?;
                        let reader = BufReader::new(stream.try_clone()?);
                        return Ok(Client::from_streams(reader, stream));
                    }
                    Err(e) => last = Some(e),
                }
            }
            let e = last.unwrap_or_else(|| io::Error::other("no address"));
            if e.kind() != io::ErrorKind::ConnectionRefused || start.elapsed() > timeout {
                return Err(Error::Io(e));
            }
            std::thread::sleep(Duration::from_millis(100));
        }
    }

    /// A client on any byte stream: responses are read from `reader`, requests written to
    /// `writer`.
    pub fn from_streams(
        reader: impl Read + Send + 'static,
        writer: impl Write + Send + 'static,
    ) -> Client {
        Client {
            reader: Box::new(BufReader::with_capacity(1 << 20, reader)),
            writer: Box::new(BufWriter::with_capacity(1 << 20, writer)),
            forwarder: None,
            label: "kasmer".into(),
            next_id: 1,
            transcript: None,
        }
    }

    /// The client stops `child` when it is dropped (after closing the streams).
    pub(crate) fn with_forwarder(mut self, child: Child) -> Client {
        self.forwarder = Some(child);
        self
    }

    /// Request ids are `<label>-<number>`; the server writes them in its logs.
    pub fn set_label(&mut self, label: impl Into<String>) {
        self.label = label.into();
    }

    /// Writes every exchange to `w`, one line each: `{"request": ..., "response": ...}`, the
    /// format of the reference data of `scripts/rpc-corpus.sh`.
    pub fn set_transcript(&mut self, w: impl Write + Send + 'static) {
        self.transcript = Some(Box::new(BufWriter::new(w)));
    }

    /// Sends a request and reads the response: its result, or the server's error.
    pub fn request<P: Serialize, R: DeserializeOwned>(
        &mut self,
        method: &str,
        params: &P,
    ) -> Result<R, Error> {
        let id = format!("{}-{:03}", self.label, self.next_id);
        self.next_id += 1;
        let request = Request {
            jsonrpc: "2.0",
            id: &id,
            method,
            params,
        };
        let mut line = Vec::new();
        kore::json::to_writer(&mut line, &request)?;
        self.writer.write_all(&line)?;
        self.writer.write_all(b"\n")?;
        self.writer.flush()?;

        let mut text = String::new();
        if self.reader.read_line(&mut text)? == 0 {
            return Err(Error::Protocol(format!(
                "{method}: the server closed the connection"
            )));
        }
        let text = text.trim_end();
        if let Some(t) = &mut self.transcript {
            t.write_all(b"{\"request\": ")?;
            t.write_all(&line)?;
            t.write_all(b", \"response\": ")?;
            t.write_all(text.as_bytes())?;
            t.write_all(b"}\n")?;
            t.flush()?;
        }
        let response: Response<R> = kore::json::from_str(text)?;
        if let Some(e) = response.error {
            return Err(Error::Rpc(e));
        }
        if response.id.as_deref() != Some(id.as_str()) {
            return Err(Error::Protocol(format!(
                "{method}: response to {:?}, expected {id}",
                response.id
            )));
        }
        response
            .result
            .ok_or_else(|| Error::Protocol(format!("{method}: response without result")))
    }

    /// A request with any parameters, and the result as JSON. For replaying recorded requests.
    ///
    /// Dropping a deep [`Value`] recurses: run on a thread with a large stack.
    pub fn request_value(&mut self, method: &str, params: &Value) -> Result<Value, Error> {
        self.request(method, params)
    }

    pub fn execute(&mut self, params: &ExecuteParams) -> Result<ExecuteResult, Error> {
        self.request("execute", params)
    }

    pub fn implies(&mut self, params: &ImpliesParams) -> Result<ImpliesResult, Error> {
        self.request("implies", params)
    }

    pub fn simplify(&mut self, params: &SimplifyParams) -> Result<SimplifyResult, Error> {
        self.request("simplify", params)
    }

    pub fn get_model(&mut self, params: &GetModelParams) -> Result<GetModelResult, Error> {
        self.request("get-model", params)
    }

    pub fn add_module(&mut self, params: &AddModuleParams) -> Result<AddModuleResult, Error> {
        self.request("add-module", params)
    }
}

impl Drop for Client {
    fn drop(&mut self) {
        let _ = self.writer.flush();
        // Closing the forwarder's standard input ends the connection.
        self.writer = Box::new(io::sink());
        self.reader = Box::new(io::empty());
        if let Some(mut child) = self.forwarder.take() {
            let deadline = Instant::now() + Duration::from_secs(5);
            while Instant::now() < deadline {
                if let Ok(Some(_)) = child.try_wait() {
                    return;
                }
                std::thread::sleep(Duration::from_millis(20));
            }
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

#[derive(serde::Deserialize)]
struct Response<R> {
    #[serde(default)]
    id: Option<String>,
    #[serde(default = "Option::default")]
    result: Option<R>,
    #[serde(default)]
    error: Option<RpcError>,
}
