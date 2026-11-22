//! The two operations of a proof on the server: extend a node by executing from it, and check
//! whether a node implies another (to cover it). The proof loop around them (which node next,
//! cut points, abstraction, when to save) belongs to the semantics using the graph.

use kore::{Conn, Ml, Pattern, Var};
use kore_rpc::Client;
use kore_rpc::protocol::{ExecuteParams, ImpliesParams, Kore};

use crate::cterm::{CSubst, CTerm, conjuncts, top_sort};
use crate::extend::{ExtendError, Extension, apply, classify};
use crate::graph::{GraphError, Kcfg, NodeId};
use crate::rules::RuleLabels;

#[derive(Debug)]
pub enum ExploreError {
    Rpc(kore_rpc::Error),
    Extend(ExtendError),
}

impl std::fmt::Display for ExploreError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ExploreError::Rpc(e) => e.fmt(f),
            ExploreError::Extend(e) => e.fmt(f),
        }
    }
}

impl std::error::Error for ExploreError {}

impl From<kore_rpc::Error> for ExploreError {
    fn from(e: kore_rpc::Error) -> ExploreError {
        ExploreError::Rpc(e)
    }
}

impl From<ExtendError> for ExploreError {
    fn from(e: ExtendError) -> ExploreError {
        ExploreError::Extend(e)
    }
}

impl From<GraphError> for ExploreError {
    fn from(e: GraphError) -> ExploreError {
        ExploreError::Extend(ExtendError::Graph(e))
    }
}

/// Executes from a leaf with the options of `params` (its `state` is replaced by the node's),
/// adds the first extension to the graph and returns all of them with the new nodes.
/// Successful rewrites are always logged, for the rule names of the edges.
pub fn extend_node(
    client: &mut Client,
    kcfg: &mut Kcfg,
    node: NodeId,
    params: &ExecuteParams,
    labels: &RuleLabels,
) -> Result<(Vec<Extension>, Vec<NodeId>), ExploreError> {
    let cterm = kcfg.node(node)?.cterm.clone();
    extend_node_as(client, kcfg, node, &cterm, params, labels)
}

/// As [`extend_node`], but executes `cterm` in place of the node's own term: a version of it with
/// parts abstracted, say. Case splits are made on the node's own term; the other new nodes come
/// from the server's results, so from `cterm`.
pub fn extend_node_as(
    client: &mut Client,
    kcfg: &mut Kcfg,
    node: NodeId,
    cterm: &CTerm,
    params: &ExecuteParams,
    labels: &RuleLabels,
) -> Result<(Vec<Extension>, Vec<NodeId>), ExploreError> {
    if !kcfg.is_leaf(node) {
        return Err(GraphError(format!("node {node} is not a leaf")).into());
    }
    let mut params = params.clone();
    params.state = Kore(cterm.to_kore());
    params.log_successful_rewrites = Some(true);
    let result = client.execute(&params)?;
    let extensions = classify(&result, labels)?;
    let new = match extensions.first() {
        Some(e) => apply(kcfg, node, e.clone())?,
        None => vec![],
    };
    Ok((extensions, new))
}

/// Whether `antecedent` implies `consequent`, and under which condition. Variables of the
/// consequent that are not in the antecedent are bound existentially, as pyk does.
pub fn implies(
    client: &mut Client,
    antecedent: &CTerm,
    consequent: &CTerm,
) -> Result<Option<CSubst>, ExploreError> {
    let bound = free_vars(&antecedent.to_kore());
    let mut consequent_kore = consequent.to_kore();
    let unbound: Vec<Var> = free_vars(&consequent_kore)
        .into_iter()
        .filter(|v| !bound.contains(v))
        .collect();
    for v in unbound {
        consequent_kore = Pattern::Ml(
            Ml::new(
                Conn::Exists,
                vec![top_sort()],
                Some(v),
                vec![consequent_kore],
            )
            .expect("well-formed \\exists"),
        );
    }
    let result = client.implies(&ImpliesParams::new(antecedent.to_kore(), consequent_kore))?;
    if !result.is_valid() {
        return Ok(None);
    }
    let condition = result.condition.unwrap_or(kore_rpc::protocol::Condition {
        substitution: None,
        predicate: None,
    });
    Ok(Some(CSubst {
        substitution: condition.substitution.map(|s| s.0),
        constraints: condition
            .predicate
            .map(|p| conjuncts(p.0))
            .unwrap_or_default(),
    }))
}

/// If `node` implies `target`, covers it and returns the condition.
pub fn try_cover(
    client: &mut Client,
    kcfg: &mut Kcfg,
    node: NodeId,
    target: NodeId,
) -> Result<Option<CSubst>, ExploreError> {
    let csubst = implies(client, &kcfg.node(node)?.cterm, &kcfg.node(target)?.cterm)?;
    if let Some(c) = &csubst {
        kcfg.create_cover(node, target, c.clone())?;
    }
    Ok(csubst)
}

/// The free element variables of a pattern, each once, in the order they first occur.
pub fn free_vars(p: &Pattern) -> Vec<Var> {
    let mut out: Vec<Var> = Vec::new();
    // (pattern, variables bound around it); children pushed in reverse to visit left to right.
    let mut todo: Vec<(&Pattern, Vec<&Var>)> = vec![(p, vec![])];
    while let Some((p, bound)) = todo.pop() {
        match p {
            Pattern::EVar(v) => {
                if !bound.contains(&v) && !out.contains(v) {
                    out.push(v.clone());
                }
            }
            Pattern::Ml(Ml {
                var: Some(v), args, ..
            }) => {
                let mut inner = bound.clone();
                inner.push(v);
                todo.extend(args.iter().rev().map(|a| (a, inner.clone())));
            }
            _ => todo.extend(p.args().iter().rev().map(|a| (a, bound.clone()))),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use kore::text::parse_pattern;

    #[test]
    fn free_variables() {
        let p = parse_pattern(
            r"f{}(X : SortInt{}, \exists{S}(Y : SortInt{}, g{}(Y : SortInt{}, Z : SortInt{})))",
        )
        .unwrap();
        let names: Vec<String> = free_vars(&p).iter().map(|v| v.name.to_string()).collect();
        assert_eq!(names, ["X", "Z"]);
    }
}
