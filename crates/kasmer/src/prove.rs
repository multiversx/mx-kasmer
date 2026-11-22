//! Symbolic proof of a claim on `kore-rpc-booster`, as `kmxwasm.property_testing.running.run_claim`
//! (`kasmer verify`).
//!
//! The proof graph starts with the claim's two nodes, `init` and `target`. In rounds, every leaf
//! that is not the target, stuck or vacuous is processed, lowest id first:
//!
//! - if its next command instantiates a Wasm module, the LLVM interpreter does that
//!   ([`crate::wasm_init`]);
//! - if its next step uses an abstracted cell, one step is executed on the server, as it is;
//! - otherwise, with the abstracted cells replaced by variables ([`crate::abstraction`]), the
//!   server executes until a branch, a cut point, a stuck state or the depth limit; the new nodes
//!   get the cells back.
//!
//! Each new leaf that could be the target (same first item in `<k>`, `<commands>` and `<instrs>`)
//! is checked against it and, if it implies it, covered. The proof succeeds when no leaf is left to
//! process, and fails at the first stuck node.
//!
//! The graph is saved after every node ([`kcfg::Store`]); a proof started again on the same
//! directory resumes where it stopped.

use std::path::Path;
use std::time::{Duration, Instant};

use kcfg::explore::{extend_node, extend_node_as, try_cover};
use kcfg::{CTerm, Kcfg, NodeId, RuleLabels, Store};
use kore::Pattern;
use kore_rpc::Client;
use kore_rpc::protocol::ExecuteParams;

use crate::abstraction::{Abstraction, CUT_POINT_RULES, touches_abstracted};
use crate::cells::{self, First};
use crate::claim::Claim;
use crate::wasm_init::WasmInit;
use crate::{Error, LlvmDefinition};

/// The options `kmxwasm` starts `kore-rpc-booster` with.
pub const BOOSTER_ARGS: [&str; 12] = [
    "--index-cells",
    "instrs,commands,k",
    "--smt-timeout",
    "1000",
    "--equation-max-iterations",
    "1000",
    "--equation-max-recursion",
    "100",
    "--interim-simplification",
    "2000",
    "--no-fallback-simplify",
    "--no-post-exec-simplify",
];

#[derive(Clone, Debug)]
pub struct Options {
    /// Rewrite steps per request at most.
    pub max_depth: u64,
    /// Nodes processed at most, in this run.
    pub max_iterations: usize,
    /// Start again rather than resume a saved proof.
    pub fresh: bool,
}

impl Default for Options {
    fn default() -> Options {
        Options {
            max_depth: 10_000,
            max_iterations: 10_000,
            fresh: false,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Status {
    /// Every path reaches the target.
    Proved,
    /// A path gets stuck: no rule applies, and it is not the target.
    Failed { stuck: NodeId },
    /// The iteration limit was reached first; these leaves are left.
    Incomplete { frontier: Vec<NodeId> },
}

#[derive(Debug)]
pub struct Outcome {
    pub status: Status,
    pub kcfg: Kcfg,
    pub init: NodeId,
    pub target: NodeId,
    /// Nodes processed in this run.
    pub iterations: usize,
    /// Whether the proof continued one saved before.
    pub resumed: bool,
}

/// What happens, for progress output.
#[derive(Debug)]
pub enum Event<'a> {
    Resumed {
        nodes: usize,
    },
    Processing {
        node: NodeId,
        how: How,
        next: Option<String>,
    },
    Processed {
        node: NodeId,
        new: &'a [NodeId],
        elapsed: Duration,
    },
    Covered {
        node: NodeId,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum How {
    WasmInit,
    OneStep,
    Abstracted,
}

pub struct Prover<'a> {
    pub definition: &'a LlvmDefinition,
    pub client: &'a mut Client,
    pub labels: &'a RuleLabels,
    pub options: Options,
}

impl Prover<'_> {
    pub fn prove(
        &mut self,
        claim: &Claim,
        dir: &Path,
        progress: &mut dyn FnMut(Event),
    ) -> Result<Outcome, Error> {
        if self.options.fresh && dir.exists() {
            std::fs::remove_dir_all(dir).map_err(Error::io(dir))?;
        }
        let resumed = Store::exists(dir);
        let store = Store::open(dir).map_err(store_error)?;
        let (mut kcfg, init, target) = if resumed {
            let kcfg = store.load().map_err(store_error)?;
            let (init, target) = match (kcfg.alias("init"), kcfg.alias("target")) {
                (Some(i), Some(t)) => (i, t),
                _ => {
                    return Err(Error::Proof(format!(
                        "{}: no init or target node",
                        dir.display()
                    )));
                }
            };
            if kcfg.node(init).map_err(graph_error)?.cterm != claim.init
                || kcfg.node(target).map_err(graph_error)?.cterm != claim.target
            {
                return Err(Error::Proof(format!(
                    "{}: the saved proof is of another claim (the contracts changed?); start again with --fresh",
                    dir.display()
                )));
            }
            progress(Event::Resumed { nodes: kcfg.len() });
            (kcfg, init, target)
        } else {
            let mut kcfg = Kcfg::new();
            let init = kcfg.create_node(claim.init.clone());
            let target = kcfg.create_node(claim.target.clone());
            kcfg.set_alias("init", init).map_err(graph_error)?;
            kcfg.set_alias("target", target).map_err(graph_error)?;
            store.save(&mut kcfg).map_err(store_error)?;
            (kcfg, init, target)
        };

        let mut wasm = WasmInit::new(&claim.init.config)?;
        let mut abstraction = Abstraction::new();
        let mut iterations = 0;
        let outcome = |status, kcfg, iterations| Outcome {
            status,
            kcfg,
            init,
            target,
            iterations,
            resumed,
        };
        loop {
            if let Some(&stuck) = kcfg.stuck().first() {
                return Ok(outcome(Status::Failed { stuck }, kcfg, iterations));
            }
            let frontier = kcfg.frontier(&[target]);
            if frontier.is_empty() {
                return Ok(outcome(Status::Proved, kcfg, iterations));
            }
            for node in frontier {
                if iterations == self.options.max_iterations {
                    let frontier = kcfg.frontier(&[target]);
                    return Ok(outcome(Status::Incomplete { frontier }, kcfg, iterations));
                }
                iterations += 1;
                let start = Instant::now();
                let new = self.process(&mut kcfg, node, &mut wasm, &mut abstraction, progress);
                // Save what was done, also when it failed.
                store.save(&mut kcfg).map_err(store_error)?;
                let new = new?;
                progress(Event::Processed {
                    node,
                    new: &new,
                    elapsed: start.elapsed(),
                });
                for n in new {
                    if !kcfg.is_leaf(n) || !quick_implication_check(&kcfg, n, target) {
                        continue;
                    }
                    if try_cover(self.client, &mut kcfg, n, target)
                        .map_err(explore_error)?
                        .is_some()
                    {
                        progress(Event::Covered { node: n });
                    }
                }
                store.save(&mut kcfg).map_err(store_error)?;
                if !kcfg.stuck().is_empty() {
                    break;
                }
            }
        }
    }

    /// Extends one leaf. Returns the new nodes.
    fn process(
        &mut self,
        kcfg: &mut Kcfg,
        node: NodeId,
        wasm: &mut WasmInit,
        abstraction: &mut Abstraction,
        progress: &mut dyn FnMut(Event),
    ) -> Result<Vec<NodeId>, Error> {
        let cterm = kcfg.node(node).map_err(graph_error)?.cterm.clone();
        let how = if WasmInit::applies(&cterm.config) {
            How::WasmInit
        } else if touches_abstracted(&cterm.config) {
            How::OneStep
        } else {
            How::Abstracted
        };
        progress(Event::Processing {
            node,
            how,
            next: next_step(&cterm.config),
        });
        match how {
            How::WasmInit => {
                let config = wasm.run(self.definition, &cterm.config)?;
                let next = kcfg.create_node(CTerm::new(config, cterm.constraints.clone()));
                kcfg.create_edge(node, next, 1, vec![])
                    .map_err(graph_error)?;
                Ok(vec![next])
            }
            How::OneStep => {
                let mut params = self.params(1);
                params.cut_point_rules = Some(vec![]);
                let (_, new) = extend_node(self.client, kcfg, node, &params, self.labels)
                    .map_err(explore_error)?;
                Ok(new)
            }
            How::Abstracted => {
                let mut sent = cterm.clone();
                abstraction.abstract_config(self.definition.symbols(), &mut sent.config)?;
                let params = self.params(self.options.max_depth);
                let result = extend_node_as(self.client, kcfg, node, &sent, &params, self.labels);
                // Put the cells back in the new nodes, also if the request failed.
                let new = result.as_ref().map(|(_, n)| n.clone()).unwrap_or_default();
                for &n in &new {
                    let c = &kcfg.node(n).map_err(graph_error)?.cterm;
                    let back = CTerm::new(
                        abstraction.concretize(c.config.clone()),
                        c.constraints
                            .iter()
                            .map(|p| abstraction.concretize(p.clone())),
                    );
                    if abstraction.leaks_into(&back.to_kore()) {
                        return Err(Error::Proof(format!(
                            "node {n}: an abstracted cell could not be put back"
                        )));
                    }
                    kcfg.replace_cterm(n, back).map_err(graph_error)?;
                }
                abstraction.clear();
                result.map_err(explore_error)?;
                Ok(new)
            }
        }
    }

    fn params(&self, depth: u64) -> ExecuteParams {
        // The state is the node's, filled in by extend_node.
        let mut p = ExecuteParams::new(Pattern::String(String::new()));
        p.max_depth = Some(depth);
        p.cut_point_rules = Some(CUT_POINT_RULES.iter().map(|r| r.to_string()).collect());
        p.terminal_rules = Some(vec![]);
        p.log_successful_rewrites = Some(true);
        p.log_failed_rewrites = Some(false);
        p.booster_only = Some(false);
        p
    }
}

/// Whether `node` may imply `target`, judged by the first items of `<k>`, `<commands>` and
/// `<instrs>`: as `kmxwasm.property_testing.implication.quick_implication_check`.
pub fn quick_implication_check(kcfg: &Kcfg, node: NodeId, target: NodeId) -> bool {
    let (Ok(a), Ok(c)) = (kcfg.node(node), kcfg.node(target)) else {
        return false;
    };
    ["k", "commands", "instrs"].iter().all(|cell| {
        let fa = cells::first_in_cell(&a.cterm.config, cell);
        let fc = cells::first_in_cell(&c.cterm.config, cell);
        match (fa, fc) {
            (First::Empty, First::App(_)) | (First::App(_), First::Empty) => false,
            (First::App(x), First::App(y)) => x.symbol == y.symbol,
            _ => true,
        }
    })
}

/// The first items of `<k>`, `<commands>` and `<instrs>`, for progress output.
pub fn next_step(config: &Pattern) -> Option<String> {
    let parts: Vec<String> = ["k", "commands", "instrs"]
        .iter()
        .filter_map(|cell| {
            cells::first_symbol(config, cell)
                .map(|s| format!("<{cell}> {}", s.trim_start_matches("Lbl")))
        })
        .collect();
    (!parts.is_empty()).then(|| parts.join(", "))
}

fn store_error(e: kcfg::store::StoreError) -> Error {
    Error::Proof(e.to_string())
}

fn graph_error(e: kcfg::graph::GraphError) -> Error {
    Error::Proof(e.to_string())
}

fn explore_error(e: kcfg::explore::ExploreError) -> Error {
    Error::Proof(e.to_string())
}
