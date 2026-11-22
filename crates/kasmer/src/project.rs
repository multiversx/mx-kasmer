//! Test projects, as `kmxwasm.kasmer.utils` reads them.
//!
//! A project is the directory of a test contract built with `sc-meta`: its `output/` holds one
//! `.wasm` file and its ABI. `kasmer.json` lists the contracts under test, each a directory built
//! the same way, relative to the project:
//!
//! ```json
//! { "contracts": ["../../examples/adder", { "path": "../../examples/multisig", "name": "multisig" }] }
//! ```
//!
//! With a `name`, the contract is `output/<name>.wasm`; without, the only `.wasm` file in `output/`.

use std::path::{Path, PathBuf};

use serde_json::Value;

use crate::Error;

pub const PROJECT_FILE: &str = "kasmer.json";
pub const TEST_PREFIX: &str = "test_";

#[derive(Clone, Debug)]
pub struct Project {
    pub dir: PathBuf,
    /// The test contract.
    pub test_wasm: PathBuf,
    /// The contracts under test.
    pub contracts: Vec<PathBuf>,
    pub endpoints: Vec<Endpoint>,
}

/// A test endpoint of the test contract: a name starting with `test_`, and its argument types as
/// the ABI writes them (`BigUint`, `u32`, ...).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Endpoint {
    pub name: String,
    pub inputs: Vec<String>,
}

impl Project {
    /// Reads `kasmer.json` and the test contract's ABI. Paths are made absolute, as in the Python
    /// tooling; they end up in the configuration as module names.
    pub fn load(dir: impl AsRef<Path>) -> Result<Project, Error> {
        let dir = canonical(dir.as_ref())?;
        let file = dir.join(PROJECT_FILE);
        let json = read_json(&file)?;
        let contracts = json
            .get("contracts")
            .and_then(Value::as_array)
            .ok_or_else(|| Error::Project(format!("{}: no \"contracts\" list", file.display())))?
            .iter()
            .map(|c| contract_wasm(&dir, c, &file))
            .collect::<Result<_, _>>()?;
        Ok(Project {
            test_wasm: only_wasm(&dir)?,
            endpoints: test_endpoints(&dir)?,
            contracts,
            dir,
        })
    }
}

fn canonical(path: &Path) -> Result<PathBuf, Error> {
    path.canonicalize().map_err(Error::io(path))
}

fn read_json(path: &Path) -> Result<Value, Error> {
    let text = std::fs::read_to_string(path).map_err(Error::io(path))?;
    serde_json::from_str(&text).map_err(|e| Error::Project(format!("{}: {e}", path.display())))
}

fn contract_wasm(project_dir: &Path, contract: &Value, file: &Path) -> Result<PathBuf, Error> {
    let (path, name) = match contract {
        Value::String(path) => (path.as_str(), None),
        Value::Object(o) => match o.get("path").and_then(Value::as_str) {
            Some(path) => (path, o.get("name").and_then(Value::as_str)),
            None => {
                return Err(Error::Project(format!(
                    "{}: contract without \"path\"",
                    file.display()
                )));
            }
        },
        _ => {
            return Err(Error::Project(format!(
                "{}: contract is neither a path nor an object",
                file.display()
            )));
        }
    };
    let dir = canonical(&project_dir.join(path))?;
    match name {
        Some(name) => {
            let wasm = dir.join("output").join(format!("{name}.wasm"));
            if !wasm.is_file() {
                return Err(Error::Project(format!("{}: not found", wasm.display())));
            }
            Ok(wasm)
        }
        None => only_wasm(&dir),
    }
}

fn files_with_suffix(dir: &Path, suffix: &str) -> Result<Vec<PathBuf>, Error> {
    let output = dir.join("output");
    let mut files = Vec::new();
    for entry in std::fs::read_dir(&output).map_err(Error::io(&output))? {
        let path = entry.map_err(Error::io(&output))?.path();
        if path
            .file_name()
            .and_then(|n| n.to_str())
            .is_some_and(|n| n.ends_with(suffix))
        {
            files.push(path);
        }
    }
    files.sort();
    Ok(files)
}

/// The only `.wasm` file in `<dir>/output`.
fn only_wasm(dir: &Path) -> Result<PathBuf, Error> {
    let mut files = files_with_suffix(dir, ".wasm")?;
    if files.len() != 1 {
        return Err(Error::Project(format!(
            "expected exactly one .wasm file in {}/output, found {}",
            dir.display(),
            files.len()
        )));
    }
    Ok(files.remove(0))
}

/// The `test_*` endpoints in the ABI in `<dir>/output`, in ABI order.
pub fn test_endpoints(dir: &Path) -> Result<Vec<Endpoint>, Error> {
    let abi_path = files_with_suffix(dir, ".abi.json")?
        .into_iter()
        .next()
        .ok_or_else(|| {
            Error::Project(format!(
                "ABI file not found: {}/output/*.abi.json",
                dir.display()
            ))
        })?;
    let abi = read_json(&abi_path)?;
    let bad = || Error::Project(format!("{}: unexpected ABI structure", abi_path.display()));
    let mut endpoints = Vec::new();
    for e in abi
        .get("endpoints")
        .and_then(Value::as_array)
        .ok_or_else(bad)?
    {
        let name = e.get("name").and_then(Value::as_str).ok_or_else(bad)?;
        if !name.starts_with(TEST_PREFIX) {
            continue;
        }
        let inputs = e
            .get("inputs")
            .and_then(Value::as_array)
            .ok_or_else(bad)?
            .iter()
            .map(|i| {
                i.get("type")
                    .and_then(Value::as_str)
                    .map(str::to_owned)
                    .ok_or_else(bad)
            })
            .collect::<Result<_, _>>()?;
        endpoints.push(Endpoint {
            name: name.to_owned(),
            inputs,
        });
    }
    Ok(endpoints)
}
