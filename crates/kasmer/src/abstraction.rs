//! Cell abstraction, as `kmxwasm.property_testing.cell_abstracter`: before a node is sent to the
//! server, large cells that the next steps do not touch are replaced by fresh variables, and put
//! back in every node that comes out. This keeps the requests small and the server from
//! simplifying what does not change.
//!
//! The cells: `<callStack>` and `<interimStates>` (saved call states and world states), and the
//! `<code>` of every account (the Wasm modules). A node about to use one of them (see
//! [`touches_abstracted`]) is executed one step at a time, without abstraction; the rules that use
//! them are cut points, so that execution stops before them.

use std::collections::HashMap;

use kore::symbols::Symbols;
use kore::{Pattern, Sort};

use crate::cells::{self, cell_symbol};
use crate::claim::var_name;
use crate::{Error, symbol_of};

/// The cut-point rules, in the order and with the repetitions of `kmxwasm` (the server does not
/// mind).
pub const CUT_POINT_RULES: [&str; 24] = [
    // <callStack>
    "ELROND-NODE.pushCallState",
    "ELROND-NODE.popCallState",
    "ELROND-NODE.dropCallState",
    "KASMER.endFoundryImmediately",
    // <interimStates>
    "ELROND-NODE.pushWorldState",
    "ELROND-NODE.popWorldState",
    "ELROND-NODE.dropWorldState",
    "KASMER.endFoundryImmediately",
    // <code>
    "ELROND-CONFIG.setAccountFields-instrs-empty",
    "ELROND-CONFIG.setAccountFields-instrs-wait",
    "ELROND-CONFIG.setAccountCode-instrs-empty",
    "ELROND-CONFIG.setAccountCode-instrs-wait",
    "ELROND-CONFIG.callContractAux",
    "ELROND-CONFIG.callContractAux-not-contract",
    "ESDT.determineIsSCCallAfter-call",
    "MANDOS.checkAccountCodeAux-no-code",
    "MANDOS.checkAccountCodeAux-code",
    "BASEOPS.checkIsSmartContract-code",
    "BASEOPS.checkIsSmartContract-no-code",
    "ELROND-CONFIG.createAccount-new-instrs-empty",
    "ELROND-CONFIG.createAccount-new-instrs-wait",
    "ELROND-NODE.pushWorldState",
    "ELROND-NODE.popWorldState",
    // Wasm instantiation, done on the LLVM interpreter instead (see wasm_init).
    "ELROND-CONFIG.newWasmInstance",
];

/// Whether the next step of a configuration may use an abstracted cell: by the first item of
/// `<k>`, `<commands>` and `<instrs>`, as `kmxwasm.ast.mx.cfg_changes_call_stack`,
/// `cfg_changes_interim_states` and `cfg_touches_code`.
pub fn touches_abstracted(config: &Pattern) -> bool {
    let k = cells::first_symbol(config, "k");
    let command = cells::first_symbol(config, "commands");
    let instr = cells::first_symbol(config, "instrs");
    let is = |s: Option<&str>, labels: &[&str]| {
        s.is_some_and(|s| labels.iter().any(|l| symbol_of(l) == s))
    };
    is(
        command,
        &[
            "pushCallState",
            "popCallState",
            "dropCallState",
            "pushWorldState",
            "popWorldState",
            "dropWorldState",
            "setAccountFields",
            "setAccountCode",
            "callContractAux",
            "createAccount",
            "determineIsSCCallAfter",
        ],
    ) || is(instr, &["endFoundryImmediately", "checkIsSmartContract"])
        || is(k, &["checkAccountCodeAux"])
}

/// The values taken out of a configuration, by variable name. Variables are numbered across the
/// whole proof, so that names do not repeat.
#[derive(Default)]
pub struct Abstraction {
    counters: HashMap<&'static str, usize>,
    values: HashMap<String, Pattern>,
}

impl Abstraction {
    pub fn new() -> Abstraction {
        Abstraction::default()
    }

    fn fresh(&mut self, root: &'static str, sort: Sort) -> Pattern {
        let n = self.counters.entry(root).or_default();
        let name = var_name(&format!("{root}_{n}"));
        *n += 1;
        Pattern::evar(name, sort)
    }

    /// Replaces the abstracted cells of `config` by variables, remembering their contents.
    pub fn abstract_config(
        &mut self,
        symbols: &Symbols,
        config: &mut Pattern,
    ) -> Result<(), Error> {
        for (cell, root) in [
            ("callStack", "AbstractCallStack"),
            ("interimStates", "AbstractInterimStates"),
        ] {
            let sort = content_sort(symbols, cell)?;
            let slot = cells::cell_mut(config, cell)
                .ok_or_else(|| Error::Project(format!("no cell <{cell}>")))?;
            if worth_abstracting(slot) {
                let var = self.fresh(root, sort);
                self.remember(slot, var);
            }
        }
        // <code>: an injection of a module into `Code`; the module becomes a variable of its sort.
        // `.Code` is left alone (pyk abstracts it too, as a module, which it is not).
        cells::for_each_cell_mut(config, "code", |slot| {
            if let Pattern::App(inj) = slot
                && &*inj.symbol == "inj"
                && inj.args.len() == 1
                && worth_abstracting(&inj.args[0])
            {
                let var = self.fresh("AbstractCode", inj.sorts[0].clone());
                self.remember(&mut inj.args[0], var);
            }
        });
        Ok(())
    }

    fn remember(&mut self, slot: &mut Pattern, var: Pattern) {
        let Pattern::EVar(v) = &var else {
            unreachable!()
        };
        let name = v.name.to_string();
        let value = std::mem::replace(slot, var);
        self.values.insert(name, value);
    }

    /// Puts the contents back in place of their variables, wherever they are.
    pub fn concretize(&self, config: Pattern) -> Pattern {
        if self.values.is_empty() {
            return config;
        }
        config.map_bottom_up(|p| match &p {
            Pattern::EVar(v) => match self.values.get(&*v.name) {
                Some(value) => value.clone(),
                None => p,
            },
            _ => p,
        })
    }

    /// Whether a pattern still has one of the variables.
    pub fn leaks_into(&self, p: &Pattern) -> bool {
        let mut found = false;
        p.visit(|q| {
            if let Pattern::EVar(v) = q
                && self.values.contains_key(&*v.name)
            {
                found = true;
            }
        });
        found
    }

    /// Forgets the contents: they belong to the node just processed.
    pub fn clear(&mut self) {
        self.values.clear();
    }
}

/// Small or already symbolic contents stay: tokens and variables.
fn worth_abstracting(p: &Pattern) -> bool {
    !matches!(p, Pattern::Dv { .. } | Pattern::EVar(_))
}

fn content_sort(symbols: &Symbols, cell: &str) -> Result<Sort, Error> {
    symbols
        .signature(&cell_symbol(cell))
        .and_then(|s| s.params.first().cloned())
        .ok_or_else(|| Error::Project(format!("no cell <{cell}> in the definition")))
}
