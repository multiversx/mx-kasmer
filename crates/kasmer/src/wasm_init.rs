//! Instantiating a Wasm module concretely, as `kmxwasm.property_testing.wasm_krun_initializer`:
//! when the next command of a node is `newWasmInstance(address, module)`, the module is
//! instantiated by the LLVM interpreter rather than symbolically, which is much faster. Code is
//! concrete, so the result does not depend on the path condition.
//!
//! The interpreter runs the command alone, in a configuration emptied of everything symbolic
//! (accounts, call stack, block information, ...) and with the `<wasm>` cell of the first node of
//! the proof. The `<wasm>` and `<contractModIdx>` cells of the result replace those of the node,
//! the command is dropped, and that is the next node. Results are cached by contract address.

use std::collections::HashMap;

use kore::Pattern;

use crate::cells::{self, First, set_cell};
use crate::setup::stuck;
use crate::steps::Builder;
use crate::{Error, LlvmDefinition, symbol_of};

pub struct WasmInit {
    /// The `<wasm>` cell of the first node of the proof.
    first_wasm: Pattern,
    /// Address to `<wasm>` and `<contractModIdx>`.
    cache: HashMap<String, (Pattern, Pattern)>,
}

impl WasmInit {
    pub fn new(first_config: &Pattern) -> Result<WasmInit, Error> {
        Ok(WasmInit {
            first_wasm: cells::whole_cell(first_config, "wasm")
                .ok_or_else(|| Error::Project("no cell <wasm>".into()))?
                .clone(),
            cache: HashMap::new(),
        })
    }

    /// Whether the next command of a configuration instantiates a module.
    pub fn applies(config: &Pattern) -> bool {
        cells::first_symbol(config, "commands") == Some(&symbol_of("newWasmInstance"))
    }

    /// The configuration after the module is instantiated.
    pub fn run(&mut self, definition: &LlvmDefinition, config: &Pattern) -> Result<Pattern, Error> {
        let commands = cells::cell(config, "commands")
            .ok_or_else(|| Error::Project("no cell <commands>".into()))?;
        let First::App(command) = cells::first_item(commands) else {
            return Err(Error::Project("no command to instantiate a module".into()));
        };
        let address = match command.args.first() {
            Some(Pattern::Dv { value, .. }) => value.clone(),
            other => {
                return Err(Error::Project(format!(
                    "newWasmInstance with a symbolic address: {other:?}"
                )));
            }
        };
        let (wasm, mod_idx) = match self.cache.get(&address) {
            Some(cached) => cached.clone(),
            None => {
                let first = match commands {
                    Pattern::App(seq) => seq.args[0].clone(),
                    _ => unreachable!("a sequence with a first item"),
                };
                let input = self.isolated(definition, config, first)?;
                let (_, output) = definition.interpreter().run(&input)?;
                let output = output.desugar_assoc();
                if cells::first_in_cell(&output, "commands") != First::Empty {
                    return Err(stuck(&output, "instantiating a Wasm module"));
                }
                let get = |name: &str| {
                    cells::whole_cell(&output, name)
                        .cloned()
                        .ok_or_else(|| Error::Project(format!("no cell <{name}> in the output")))
                };
                let result = (get("wasm")?, get("contractModIdx")?);
                self.cache.insert(address, result.clone());
                result
            }
        };
        let mut next = config.clone();
        cells::set_whole_cell(&mut next, "wasm", wasm)?;
        cells::set_whole_cell(&mut next, "contractModIdx", mod_idx)?;
        let rest = cells::rest_of_sequence(commands)
            .cloned()
            .expect("a sequence with a first item");
        set_cell(&mut next, "commands", rest)?;
        Ok(next)
    }

    /// The configuration to run `command` in, as `WasmKrunInitializer.__prepare_krun_cell`.
    fn isolated(
        &self,
        definition: &LlvmDefinition,
        config: &Pattern,
        command: Pattern,
    ) -> Result<Pattern, Error> {
        let b = Builder::new(definition.symbols());
        let empty = |label: &str| b.app(label, vec![]).map(|t| t.pattern);
        let zero = || b.int(0).pattern;
        let mut c = config.clone();
        set_cell(&mut c, "callStack", empty(".List")?)?;
        set_cell(&mut c, "k", Pattern::app("dotk", vec![], vec![]))?;
        set_cell(
            &mut c,
            "commands",
            Pattern::app(
                "kseq",
                vec![],
                vec![command, Pattern::app("dotk", vec![], vec![])],
            ),
        )?;
        set_cell(&mut c, "instrs", Pattern::app("dotk", vec![], vec![]))?;
        set_cell(&mut c, "interimStates", empty(".List")?)?;
        set_cell(&mut c, "accounts", empty(".AccountCellMap")?)?;
        set_cell(
            &mut c,
            "logging",
            Pattern::dv(kore::Sort::app0("SortString"), ""),
        )?;
        set_cell(&mut c, "generatedCounter", zero())?;
        set_cell(&mut c, "callArgs", empty(".ListBytes")?)?;
        set_cell(&mut c, "bigIntHeap", empty(".Map")?)?;
        set_cell(&mut c, "bufferHeap", empty(".Map")?)?;
        set_cell(&mut c, "exit-code", zero())?;
        for cell in [
            "curBlockTimestamp",
            "curBlockNonce",
            "curBlockRound",
            "curBlockEpoch",
        ] {
            set_cell(&mut c, cell, zero())?;
        }
        set_cell(&mut c, "outputAccounts", empty(".Map")?)?;
        cells::set_whole_cell(&mut c, "wasm", self.first_wasm.clone())?;
        Ok(c)
    }
}
