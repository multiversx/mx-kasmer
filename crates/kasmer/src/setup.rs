//! The configurations a test starts from, as `kmultiversx.kasmer.deploy_test` builds them.
//!
//! 1. [`empty_config`]: the initial configuration of the definition.
//! 2. [`runtime_config`]: plus the ESDT system smart contract, which the semantics calls for ESDT
//!    operations. (The Python tooling keeps this one precomputed, `kdist/runtime/llvm-kasmer.json`.)
//! 3. [`deploy`]: plus the account `address:k`, which deploys the test contract at `sc:k-test`.
//!    The test contract receives the names of the contracts under test as deploy arguments, and
//!    deploys them from `<wasmStore>`, where they are stored under these names.
//!
//! The configurations are returned with collections as nested binary applications, as pyk gives
//! them, rather than in the `\left-assoc` form the interpreter prints
//! ([`Pattern::desugar_assoc`]). The interpreter reads both.

use std::path::Path;

use kore::Pattern;
use kore::symbols::Term;

use crate::cells::{self, set_cell};
use crate::steps::Builder;
use crate::{Error, LlvmDefinition, mandos};

/// The account that deploys the test contract and calls its endpoints: `address:k`.
pub const ROOT_ACCOUNT: &str = "k";
/// The test contract: `sc:k-test`.
pub const TEST_CONTRACT: &str = "k-test";

/// Gas limit of the deployment of the test contract.
pub const DEPLOY_GAS: u64 = 5_000_000_000_000;

/// The ESDT system smart contract, `contracts/esdt-system-sc`.
pub const ESDT_SYSTEM_SC_WASM: &[u8] =
    include_bytes!("../../../contracts/esdt-system-sc/output/esdt-system-sc.wasm");
/// The name stored in its module.
pub const ESDT_SYSTEM_SC_NAME: &str = "esdt-system-sc.wasm";
/// Its address: `0x000000000000000000010000000000000000000000000000000000000002ffff`.
pub const ESDT_SYSTEM_SC_ADDRESS: [u8; 32] = {
    let mut a = [0u8; 32];
    a[9] = 1;
    a[30] = 0xff;
    a[31] = 0xff;
    a[29] = 2;
    a
};

/// A contract: its code, and the name it is known by in the configuration (its path, in the
/// Python tooling: the file name stored in its module, and its key in `<wasmStore>`).
#[derive(Clone, Debug)]
pub struct Contract {
    pub name: String,
    pub wasm: Vec<u8>,
}

impl Contract {
    /// Reads a `.wasm` file; the name is the path as given.
    pub fn read(path: impl AsRef<Path>) -> Result<Contract, Error> {
        let path = path.as_ref();
        Ok(Contract {
            name: path.display().to_string(),
            wasm: std::fs::read(path).map_err(Error::io(path))?,
        })
    }

    pub fn esdt_system_sc() -> Contract {
        Contract {
            name: ESDT_SYSTEM_SC_NAME.into(),
            wasm: ESDT_SYSTEM_SC_WASM.to_vec(),
        }
    }

    /// The module, as a term of sort `ModuleDecl`.
    pub fn module(&self, definition: &LlvmDefinition) -> Result<Term, Error> {
        kwasm::module_term(&self.wasm, Some(&self.name), definition.symbols()).map_err(|source| {
            Error::Wasm {
                name: self.name.clone(),
                source,
            }
        })
    }
}

/// The initial configuration: `initGeneratedTopCell` with `$PGM` bound to no steps, evaluated by
/// the interpreter without taking any rewrite step.
pub fn empty_config(definition: &LlvmDefinition) -> Result<Pattern, Error> {
    let b = Builder::new(definition.symbols());
    let pgm = Term::dv(kore::Sort::app0("SortKConfigVar"), "$PGM");
    let vars = b.map(vec![(pgm, b.app(".List{\"mandosSteps\"}", vec![])?)])?;
    let init = b.app("initGeneratedTopCell", vec![vars])?;
    let output = definition
        .interpreter()
        .run_text(&init.pattern.to_string(), Some(0))?;
    Ok(output.config()?.desugar_assoc())
}

/// Runs a configuration to the end, and checks that all of `<k>` was consumed.
pub fn run_to_completion(
    definition: &LlvmDefinition,
    config: &Pattern,
    what: &str,
) -> Result<Pattern, Error> {
    let (_, config) = definition.interpreter().run(config)?;
    if !cells::k_is_empty(&config) {
        return Err(stuck(&config, what));
    }
    Ok(config.desugar_assoc())
}

pub(crate) fn stuck(config: &Pattern, what: &str) -> Error {
    Error::Stuck {
        what: what.into(),
        k: cells::cell_text(config, "k"),
        vm_output: cells::cell_text(config, "vmOutput"),
        logging: cells::cell_text(config, "logging"),
    }
}

/// The empty configuration plus the ESDT system smart contract.
pub fn runtime_config(
    definition: &LlvmDefinition,
    esdt_system_sc: &Contract,
) -> Result<Pattern, Error> {
    let b = Builder::new(definition.symbols());
    let mut config = empty_config(definition)?;
    let set = b.set_account(
        &ESDT_SYSTEM_SC_ADDRESS,
        0,
        0,
        Some(esdt_system_sc.module(definition)?),
        &[],
        b.map(vec![])?,
    )?;
    set_cell(&mut config, "k", b.kseq(vec![set]).pattern)?;
    run_to_completion(
        definition,
        &config,
        "setting up the ESDT system smart contract",
    )
}

/// Deploys the test contract into a runtime configuration. Returns the final configuration, with
/// `<wasmStore>` emptied again.
pub fn deploy(
    definition: &LlvmDefinition,
    mut runtime: Pattern,
    test: &Contract,
    contracts: &[Contract],
) -> Result<Pattern, Error> {
    let b = Builder::new(definition.symbols());
    let root = mandos::address(ROOT_ACCOUNT);
    let test_address = mandos::sc_address(TEST_CONTRACT);
    let names: Vec<&[u8]> = contracts.iter().map(|c| c.name.as_bytes()).collect();
    let steps = vec![
        b.set_exit_code(1)?,
        b.set_account(&root, 1, 100000, None, &[], b.map(vec![])?)?,
        b.new_address(&root, 1, &test_address)?,
        b.deploy_tx(&root, 0, test.module(definition)?, &names, DEPLOY_GAS, 0)?,
        b.check_expect_ok()?,
        b.set_exit_code(0)?,
    ];
    let store = contracts
        .iter()
        .map(|c| Ok((b.bytes(c.name.as_bytes()), c.module(definition)?)))
        .collect::<Result<Vec<_>, Error>>()?;
    set_cell(&mut runtime, "k", b.kseq(steps).pattern)?;
    set_cell(&mut runtime, "wasmStore", b.map(store)?.pattern)?;
    let mut config = run_to_completion(definition, &runtime, "deploying the test contract")?;
    set_cell(&mut config, "wasmStore", b.map(vec![])?.pattern)?;
    Ok(config)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn esdt_address() {
        let hex: String = ESDT_SYSTEM_SC_ADDRESS
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect();
        assert_eq!(
            hex,
            "000000000000000000010000000000000000000000000000000000000002ffff"
        );
    }
}
