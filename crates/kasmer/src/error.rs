use std::fmt;
use std::path::PathBuf;

use kore::symbols::BuildError;
use kore::text::ParseError;

#[derive(Debug)]
pub enum Error {
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
    /// A file that is not what it should be: `kasmer.json`, an ABI, a missing `.wasm`.
    Project(String),
    /// KORE text that does not parse: `definition.kore`, or the output of the interpreter.
    Parse {
        what: String,
        source: ParseError,
    },
    Wasm {
        name: String,
        source: kwasm::Error,
    },
    /// A term the definition does not have the symbols for.
    Build(BuildError),
    /// The interpreter could not be run, or gave no configuration.
    Interpreter(String),
    /// A run that should have consumed all the steps in `<k>` stopped before.
    Stuck {
        what: String,
        /// `<k>`, `<vmOutput>` and `<logging>` of the final configuration, in KORE.
        k: String,
        vm_output: String,
        logging: String,
    },
}

impl Error {
    pub(crate) fn io(path: impl Into<PathBuf>) -> impl FnOnce(std::io::Error) -> Error {
        let path = path.into();
        move |source| Error::Io { path, source }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Io { path, source } => write!(f, "{}: {source}", path.display()),
            Error::Project(m) => f.write_str(m),
            Error::Parse { what, source } => write!(f, "{what}: {source}"),
            Error::Wasm { name, source } => write!(f, "{name}: {source}"),
            Error::Build(e) => e.fmt(f),
            Error::Interpreter(m) => write!(f, "interpreter: {m}"),
            Error::Stuck {
                what,
                k,
                vm_output,
                logging,
            } => {
                write!(
                    f,
                    "{what}: <k> not empty\n  k: {k}\n  vm output: {vm_output}\n  logging: {logging}"
                )
            }
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Error::Io { source, .. } => Some(source),
            Error::Parse { source, .. } => Some(source),
            Error::Wasm { source, .. } => Some(source),
            Error::Build(e) => Some(e),
            _ => None,
        }
    }
}

impl From<BuildError> for Error {
    fn from(e: BuildError) -> Error {
        Error::Build(e)
    }
}
