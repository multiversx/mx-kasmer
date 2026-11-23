//! From the result of `execute` on a node to changes of the graph, as pyk's
//! `KCFGExplore.extend_cterm` and `KCFG.extend` do it.
//!
//! [`classify`] reads a result into a list of [`Extension`]s: the run of steps taken, if any,
//! then what stopped it (stuck, vacuous, a cut point, a branch). [`apply`] adds one extension
//! to the graph. pyk's prover in `kmxwasm` applies only the first of the list; the rest is
//! found again by executing from the new node.

use kore::Pattern;
use kore_rpc::protocol::{ExecuteResult, StopReason};

use crate::cterm::{CTerm, conjuncts};
use crate::graph::{GraphError, Kcfg, NodeId};
use crate::rules::RuleLabels;

#[derive(Clone, PartialEq, Debug)]
pub enum Extension {
    /// `depth` rewrite steps to `cterm`. `cut`: the step of a cut-point rule, taken after the run.
    Step {
        cterm: CTerm,
        depth: u64,
        rules: Vec<String>,
        cut: bool,
    },
    Stuck,
    Vacuous,
    /// Rules apply under different conditions: one case per branch, the constraints of its rule
    /// that not all branches share.
    Branch {
        cases: Vec<Vec<Pattern>>,
    },
    /// Rules apply that do not exclude each other: the result of each.
    NdBranch {
        cterms: Vec<CTerm>,
        rules: Vec<String>,
    },
}

#[derive(Debug)]
pub enum ExtendError {
    /// The server gave up (`aborted`), on this predicate.
    Aborted(Option<Pattern>),
    /// A result that does not have the shape its stop reason promises.
    Unexpected(String),
    Graph(GraphError),
}

impl std::fmt::Display for ExtendError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ExtendError::Aborted(Some(p)) => write!(f, "the server aborted on: {p}"),
            ExtendError::Aborted(None) => f.write_str("the server aborted"),
            ExtendError::Unexpected(m) => f.write_str(m),
            ExtendError::Graph(e) => e.fmt(f),
        }
    }
}

impl std::error::Error for ExtendError {}

impl From<GraphError> for ExtendError {
    fn from(e: GraphError) -> ExtendError {
        ExtendError::Graph(e)
    }
}

pub fn classify(
    result: &ExecuteResult,
    labels: &RuleLabels,
) -> Result<Vec<Extension>, ExtendError> {
    if result.reason == StopReason::Aborted {
        return Err(ExtendError::Aborted(
            result.unknown_predicate.as_ref().map(|p| p.0.clone()),
        ));
    }
    let rules = || labels.of_all(&result.rule_ids());
    let next = result.next_states();
    if next.len() == 1 && result.reason != StopReason::CutPointRule {
        return Err(ExtendError::Unexpected(format!(
            "one next state, but the server stopped for {:?}",
            result.reason
        )));
    }
    let mut out = Vec::new();
    if result.depth > 0 {
        out.push(Extension::Step {
            cterm: CTerm::from_state(&result.state),
            depth: result.depth,
            rules: rules(),
            cut: false,
        });
    }
    // The rules of the steps go to the first extension only.
    let rules_if_first = |out: &Vec<Extension>| if out.is_empty() { rules() } else { vec![] };
    match next {
        [] => {
            if result.reason == StopReason::Vacuous {
                out.push(Extension::Vacuous);
            } else if result.depth == 0 {
                out.push(Extension::Stuck);
            }
        }
        [one] => out.push(Extension::Step {
            cterm: CTerm::from_state(one),
            depth: 1,
            rules: rules_if_first(&out),
            cut: true,
        }),
        _ if next.iter().all(|s| s.rule_predicate.is_some()) => {
            let preds: Vec<Vec<Pattern>> = next
                .iter()
                .map(|s| conjuncts(s.rule_predicate.clone().unwrap().0))
                .collect();
            let common: Vec<&Pattern> = preds[0]
                .iter()
                .filter(|p| preds.iter().all(|ps| ps.contains(p)))
                .collect();
            let cases = preds
                .iter()
                .map(|ps| ps.iter().filter(|p| !common.contains(p)).cloned().collect())
                .collect();
            out.push(Extension::Branch { cases });
        }
        _ => {
            let rules = rules_if_first(&out);
            out.push(Extension::NdBranch {
                cterms: next.iter().map(CTerm::from_state).collect(),
                rules,
            });
        }
    }
    Ok(out)
}

/// Adds an extension of `node` to the graph. Returns the new nodes.
pub fn apply(
    kcfg: &mut Kcfg,
    node: NodeId,
    extension: Extension,
) -> Result<Vec<NodeId>, GraphError> {
    Ok(match extension {
        Extension::Step {
            cterm,
            depth,
            rules,
            ..
        } => {
            let next = kcfg.create_node(cterm);
            kcfg.create_edge(node, next, depth, rules)?;
            vec![next]
        }
        Extension::Stuck => {
            kcfg.set_stuck(node, true)?;
            vec![]
        }
        Extension::Vacuous => {
            kcfg.set_vacuous(node, true)?;
            vec![]
        }
        Extension::Branch { cases } => kcfg.split_on_constraints(node, cases)?,
        Extension::NdBranch { cterms, rules } => kcfg.create_ndbranch(node, cterms, rules)?,
    })
}
