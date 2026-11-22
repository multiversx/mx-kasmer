use std::path::{Path, PathBuf};

use kore::symbols::Symbols;
use kore::text::parse_definition;

use crate::Error;
use crate::interpreter::Interpreter;

/// A K definition compiled with the LLVM backend (`kompile --backend llvm`), such as the target
/// `mx-semantics.llvm-kasmer`: a directory with `definition.kore` and the `interpreter` executable.
///
/// Where the backend does not run natively (macOS for now), `interpreter` can be a wrapper that runs
/// the real one elsewhere, e.g. in a container (`scripts/llvm-kasmer-host.sh`).
pub struct LlvmDefinition {
    dir: PathBuf,
    symbols: Symbols,
    interpreter: Interpreter,
}

impl LlvmDefinition {
    pub fn load(dir: impl AsRef<Path>) -> Result<LlvmDefinition, Error> {
        let dir = dir.as_ref().to_owned();
        let path = dir.join("definition.kore");
        let text = std::fs::read_to_string(&path).map_err(Error::io(&path))?;
        let definition = parse_definition(&text).map_err(|source| Error::Parse {
            what: path.display().to_string(),
            source,
        })?;
        let interpreter = Interpreter::new(dir.join("interpreter"));
        Ok(LlvmDefinition {
            symbols: Symbols::new(&definition),
            interpreter,
            dir,
        })
    }

    /// From symbols and an interpreter given separately.
    pub fn from_parts(dir: PathBuf, symbols: Symbols, interpreter: Interpreter) -> LlvmDefinition {
        LlvmDefinition {
            dir,
            symbols,
            interpreter,
        }
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    pub fn symbols(&self) -> &Symbols {
        &self.symbols
    }

    pub fn interpreter(&self) -> &Interpreter {
        &self.interpreter
    }
}
