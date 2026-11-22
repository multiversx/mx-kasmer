//! The `interpreter` executable of an LLVM definition, run as a separate process.
//!
//! It is called as `interpreter /dev/stdin <depth> /dev/stdout`: KORE text in, the final
//! configuration as KORE text out. Its exit code is the content of the cell marked `exit` in the
//! configuration (`<exit-code>` in the MultiversX semantics), once no more rules apply.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use kore::Pattern;
use kore::text::parse_pattern;

use crate::Error;

#[derive(Clone, Debug)]
pub struct Interpreter {
    program: PathBuf,
}

/// What the interpreter printed, and how it exited.
#[derive(Clone, Debug)]
pub struct Output {
    /// `None` if it was killed by a signal.
    pub exit_code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
}

impl Output {
    /// The final configuration.
    pub fn config(&self) -> Result<Pattern, Error> {
        if self.stdout.trim().is_empty() {
            return Err(Error::Interpreter(format!(
                "no configuration in the output (exit code {:?}){}",
                self.exit_code,
                stderr_suffix(&self.stderr)
            )));
        }
        parse_pattern(&self.stdout).map_err(|source| Error::Parse {
            what: "interpreter output".into(),
            source,
        })
    }
}

fn stderr_suffix(stderr: &str) -> String {
    let stderr = stderr.trim();
    if stderr.is_empty() {
        String::new()
    } else {
        format!(": {stderr}")
    }
}

impl Interpreter {
    pub fn new(program: impl Into<PathBuf>) -> Interpreter {
        Interpreter {
            program: program.into(),
        }
    }

    pub fn program(&self) -> &Path {
        &self.program
    }

    /// Runs on a configuration given as KORE text. `depth` is the maximum number of rewrite steps,
    /// `None` for no limit.
    pub fn run_text(&self, input: &str, depth: Option<u64>) -> Result<Output, Error> {
        let depth = depth.map_or_else(|| "-1".to_owned(), |d| d.to_string());
        let mut child = Command::new(&self.program)
            .args(["/dev/stdin", &depth, "/dev/stdout"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| Error::Interpreter(format!("cannot run {}: {e}", self.program.display())))?;
        let mut stdin = child.stdin.take().expect("stdin is piped");
        // Written from another thread: the interpreter may start writing before it has read
        // everything, and both pipes have limited buffers.
        let output = std::thread::scope(|s| {
            let writer = s.spawn(move || stdin.write_all(input.as_bytes()));
            let output = child.wait_with_output();
            (writer.join().expect("writer thread"), output)
        });
        let output = match output {
            (_, Err(e)) => return Err(Error::Interpreter(format!("{}: {e}", self.program.display()))),
            // A failed write means the interpreter exited without reading all of it: it rejected
            // the input, and says why on stderr.
            (_, Ok(o)) => o,
        };
        Ok(Output {
            exit_code: output.status.code(),
            stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        })
    }

    /// Runs on a configuration, with no step limit, and returns the final one.
    pub fn run(&self, config: &Pattern) -> Result<(Option<i32>, Pattern), Error> {
        let output = self.run_text(&config.to_string(), None)?;
        let config = output.config()?;
        Ok((output.exit_code, config))
    }
}
