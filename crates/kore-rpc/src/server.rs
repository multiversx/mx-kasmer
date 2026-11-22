//! Starting a K JSON-RPC server and connecting to it.
//!
//! A *server directory* says how; it is written by a script for the machine at hand
//! (`scripts/kore-rpc-host.sh` for a server in the development container):
//!
//! ```text
//! <dir>/definition.kore   the definition the server runs, for building terms on this side
//! <dir>/server            starts the server: `server --server-port <port> [options...]`; the
//!                         definition, the main module and the LLVM library are fixed by the
//!                         script. Stops the server when its standard input closes.
//! <dir>/connect           optional: `connect <port>` forwards its standard input and output to
//!                         the server's port. Without it, the client connects over TCP to
//!                         127.0.0.1:<port>.
//! ```
//!
//! The server's output goes to an optional log file; its last lines are kept for error messages.
//! It is ready when it writes `Starting RPC server`, as `kore-rpc-booster` does.

use std::collections::VecDeque;
use std::fs::File;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crate::{Client, Error};

pub const DEFINITION_FILE: &str = "definition.kore";
pub const SERVER_SCRIPT: &str = "server";
pub const CONNECT_SCRIPT: &str = "connect";

/// What the server writes when it accepts connections.
const READY: &str = "Starting RPC server";
const TAIL_LINES: usize = 40;

pub struct Server {
    dir: PathBuf,
    child: Child,
    /// Closed to stop the server.
    stdin: Option<ChildStdin>,
    port: u16,
    tail: Arc<Mutex<VecDeque<String>>>,
}

#[derive(Clone, Debug)]
pub struct Options {
    /// Arguments after `--server-port <port>`: `--smt-timeout 1000`, ...
    pub args: Vec<String>,
    /// Appends everything the server writes.
    pub log: Option<PathBuf>,
    /// How long loading the definition may take.
    pub start_timeout: Duration,
}

impl Default for Options {
    fn default() -> Options {
        Options {
            args: Vec::new(),
            log: None,
            start_timeout: Duration::from_secs(600),
        }
    }
}

impl Server {
    /// Starts the server of a server directory and waits until it accepts connections.
    pub fn start(dir: impl AsRef<Path>, options: &Options) -> Result<Server, Error> {
        let dir = dir.as_ref().to_path_buf();
        let script = dir.join(SERVER_SCRIPT);
        let port = free_port()?;
        let mut child = Command::new(&script)
            .arg("--server-port")
            .arg(port.to_string())
            .args(&options.args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| Error::Protocol(format!("cannot run {}: {e}", script.display())))?;
        let log = match &options.log {
            Some(path) => Some(Arc::new(Mutex::new(
                File::options()
                    .create(true)
                    .append(true)
                    .open(path)
                    .map_err(|e| Error::Protocol(format!("{}: {e}", path.display())))?,
            ))),
            None => None,
        };
        let tail = Arc::new(Mutex::new(VecDeque::new()));
        let (ready_tx, ready_rx) = mpsc::channel();
        let stdout = child.stdout.take().expect("piped");
        let stderr = child.stderr.take().expect("piped");
        for out in [Box::new(stdout) as Box<dyn Read + Send>, Box::new(stderr)] {
            let (tail, log, ready) = (tail.clone(), log.clone(), ready_tx.clone());
            std::thread::spawn(move || follow(out, &tail, log.as_deref(), &ready));
        }
        drop(ready_tx);
        let mut server = Server {
            dir,
            stdin: child.stdin.take(),
            child,
            port,
            tail,
        };
        // Ready, or both outputs closed (the server ended), or too slow.
        match ready_rx.recv_timeout(options.start_timeout) {
            Ok(()) => Ok(server),
            Err(e) => {
                let why = match e {
                    mpsc::RecvTimeoutError::Timeout => {
                        format!("did not start within {:?}", options.start_timeout)
                    }
                    mpsc::RecvTimeoutError::Disconnected => {
                        let status = server.child.wait().map(|s| s.to_string());
                        format!("ended ({})", status.unwrap_or_else(|e| e.to_string()))
                    }
                };
                Err(Error::Protocol(format!(
                    "server {}: {why}{}",
                    script.display(),
                    server.tail_text()
                )))
            }
        }
    }

    pub fn port(&self) -> u16 {
        self.port
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// A new connection.
    pub fn connect(&self) -> Result<Client, Error> {
        let connect = self.dir.join(CONNECT_SCRIPT);
        if !connect.exists() {
            return Client::connect_tcp(("127.0.0.1", self.port), Duration::from_secs(10));
        }
        let mut child = Command::new(&connect)
            .arg(self.port.to_string())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .map_err(|e| Error::Protocol(format!("cannot run {}: {e}", connect.display())))?;
        let reader = child.stdout.take().expect("piped");
        let writer = child.stdin.take().expect("piped");
        Ok(Client::from_streams(reader, writer).with_forwarder(child))
    }

    /// The last lines the server wrote, for error messages; empty if none.
    pub fn tail_text(&self) -> String {
        let tail = self.tail.lock().unwrap();
        if tail.is_empty() {
            return String::new();
        }
        let lines: Vec<&str> = tail.iter().map(String::as_str).collect();
        format!(
            "\n  server output (last lines):\n    {}",
            lines.join("\n    ")
        )
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        // The server script stops the server when its standard input closes.
        drop(self.stdin.take());
        let deadline = Instant::now() + Duration::from_secs(10);
        while Instant::now() < deadline {
            if let Ok(Some(_)) = self.child.try_wait() {
                return;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// Copies the server's output to the log and the tail, and signals the ready line.
fn follow(
    out: Box<dyn Read + Send>,
    tail: &Mutex<VecDeque<String>>,
    log: Option<&Mutex<File>>,
    ready: &mpsc::Sender<()>,
) {
    let mut out = BufReader::new(out);
    let mut line = String::new();
    loop {
        line.clear();
        match out.read_line(&mut line) {
            Ok(0) | Err(_) => return,
            Ok(_) => {}
        }
        if let Some(log) = log {
            let _ = log.lock().unwrap().write_all(line.as_bytes());
        }
        let text = line.trim_end();
        if text.contains(READY) {
            let _ = ready.send(());
        }
        let mut tail = tail.lock().unwrap();
        if tail.len() == TAIL_LINES {
            tail.pop_front();
        }
        tail.push_back(text.to_owned());
    }
}

/// A port nothing listens on here. With the server in a container, the port is chosen on the
/// host; a clash in the container makes the server fail to start, with its message.
fn free_port() -> Result<u16, Error> {
    Ok(TcpListener::bind(("127.0.0.1", 0))?.local_addr()?.port())
}
