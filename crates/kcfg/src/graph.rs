//! The proof graph: nodes are constrained terms, each node has at most one successor.
//!
//! - **edge**: a run of rewrite steps (`depth` of them) from the source to the target;
//! - **cover**: the source is an instance of the target (an implication, with its condition),
//!   so the target's proof covers it;
//! - **split**: a case split on constraints; the targets are the source with each case added;
//! - **ndbranch**: several rules applied to the source, each target the result of one.
//!
//! Nodes can be marked stuck (no rule applies) or vacuous (unsatisfiable path condition), and
//! named by aliases. As pyk's `KCFG`, in KORE rather than KAST.
//!
//! The graph records which nodes changed or went away since it was last saved, so that
//! [`crate::store`] writes only those.

use std::collections::{BTreeMap, BTreeSet};

use kore::Pattern;

use crate::cterm::{CSubst, CTerm};

pub type NodeId = u64;

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Node {
    pub id: NodeId,
    pub cterm: CTerm,
    pub stuck: bool,
    pub vacuous: bool,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Successor {
    Edge {
        target: NodeId,
        depth: u64,
        rules: Vec<String>,
    },
    Cover {
        target: NodeId,
        csubst: CSubst,
    },
    /// Each target with the constraints that the split added to the source.
    Split {
        targets: Vec<(NodeId, Vec<Pattern>)>,
    },
    NdBranch {
        targets: Vec<NodeId>,
        rules: Vec<String>,
    },
}

impl Successor {
    pub fn targets(&self) -> Vec<NodeId> {
        match self {
            Successor::Edge { target, .. } | Successor::Cover { target, .. } => vec![*target],
            Successor::Split { targets } => targets.iter().map(|(t, _)| *t).collect(),
            Successor::NdBranch { targets, .. } => targets.clone(),
        }
    }
}

#[derive(Debug)]
pub struct GraphError(pub String);

impl std::fmt::Display for GraphError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for GraphError {}

type R<T> = Result<T, GraphError>;

fn err<T>(m: String) -> R<T> {
    Err(GraphError(m))
}

#[derive(Clone, Debug, Default)]
pub struct Kcfg {
    pub(crate) next: NodeId,
    pub(crate) nodes: BTreeMap<NodeId, Node>,
    pub(crate) successors: BTreeMap<NodeId, Successor>,
    pub(crate) aliases: BTreeMap<String, NodeId>,
    /// Created or replaced since the last save.
    pub(crate) changed: BTreeSet<NodeId>,
    /// Removed since the last save.
    pub(crate) removed: BTreeSet<NodeId>,
}

impl Kcfg {
    pub fn new() -> Kcfg {
        Kcfg {
            next: 1,
            ..Default::default()
        }
    }

    // --- nodes ------------------------------------------------------------------------------

    pub fn create_node(&mut self, cterm: CTerm) -> NodeId {
        let id = self.next;
        self.next += 1;
        self.nodes.insert(
            id,
            Node {
                id,
                cterm,
                stuck: false,
                vacuous: false,
            },
        );
        self.changed.insert(id);
        id
    }

    pub fn node(&self, id: NodeId) -> R<&Node> {
        self.nodes
            .get(&id)
            .ok_or_else(|| GraphError(format!("no node {id}")))
    }

    pub fn nodes(&self) -> impl Iterator<Item = &Node> {
        self.nodes.values()
    }

    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    /// Replaces the term of a node, keeping its edges (as abstraction and concretization do).
    pub fn replace_cterm(&mut self, id: NodeId, cterm: CTerm) -> R<()> {
        let Some(node) = self.nodes.get_mut(&id) else {
            return err(format!("no node {id}"));
        };
        node.cterm = cterm;
        self.changed.insert(id);
        Ok(())
    }

    pub fn set_stuck(&mut self, id: NodeId, stuck: bool) -> R<()> {
        let Some(node) = self.nodes.get_mut(&id) else {
            return err(format!("no node {id}"));
        };
        node.stuck = stuck;
        Ok(())
    }

    pub fn set_vacuous(&mut self, id: NodeId, vacuous: bool) -> R<()> {
        let Some(node) = self.nodes.get_mut(&id) else {
            return err(format!("no node {id}"));
        };
        node.vacuous = vacuous;
        Ok(())
    }

    /// Removes a node, the successor from it, every successor into it and its aliases.
    pub fn remove_node(&mut self, id: NodeId) -> R<()> {
        if self.nodes.remove(&id).is_none() {
            return err(format!("no node {id}"));
        }
        self.successors.remove(&id);
        self.successors.retain(|_, s| !s.targets().contains(&id));
        self.aliases.retain(|_, n| *n != id);
        self.changed.remove(&id);
        self.removed.insert(id);
        Ok(())
    }

    /// Removes a node and everything reachable from it only through it. Returns the removed nodes.
    pub fn prune(&mut self, id: NodeId, keep: &[NodeId]) -> R<Vec<NodeId>> {
        self.node(id)?;
        let mut removed = Vec::new();
        let mut todo = vec![id];
        while let Some(n) = todo.pop() {
            if keep.contains(&n) || !self.nodes.contains_key(&n) {
                continue;
            }
            let targets = self
                .successor(n)
                .map(Successor::targets)
                .unwrap_or_default();
            self.remove_node(n)?;
            removed.push(n);
            for t in targets {
                if self.predecessors(t).is_empty() {
                    todo.push(t);
                }
            }
        }
        Ok(removed)
    }

    // --- successors -------------------------------------------------------------------------

    pub fn successor(&self, id: NodeId) -> Option<&Successor> {
        self.successors.get(&id)
    }

    pub fn successors(&self) -> impl Iterator<Item = (NodeId, &Successor)> {
        self.successors.iter().map(|(s, x)| (*s, x))
    }

    /// The nodes with a successor into `id`.
    pub fn predecessors(&self, id: NodeId) -> Vec<NodeId> {
        self.successors
            .iter()
            .filter(|(_, s)| s.targets().contains(&id))
            .map(|(source, _)| *source)
            .collect()
    }

    /// Adds the successor of `source`; it must have none, and the targets must exist.
    pub fn add_successor(&mut self, source: NodeId, successor: Successor) -> R<()> {
        self.node(source)?;
        if let Some(s) = self.successors.get(&source) {
            return err(format!("node {source} already has a successor: {s:?}"));
        }
        let targets = successor.targets();
        for t in &targets {
            self.node(*t)?;
        }
        if matches!(
            successor,
            Successor::Split { .. } | Successor::NdBranch { .. }
        ) && targets.len() < 2
        {
            return err(format!("a split or branch of {source} needs two targets"));
        }
        self.successors.insert(source, successor);
        Ok(())
    }

    pub fn remove_successor(&mut self, source: NodeId) -> Option<Successor> {
        self.successors.remove(&source)
    }

    pub fn create_edge(
        &mut self,
        source: NodeId,
        target: NodeId,
        depth: u64,
        rules: Vec<String>,
    ) -> R<()> {
        self.add_successor(
            source,
            Successor::Edge {
                target,
                depth,
                rules,
            },
        )
    }

    pub fn create_cover(&mut self, source: NodeId, target: NodeId, csubst: CSubst) -> R<()> {
        self.add_successor(source, Successor::Cover { target, csubst })
    }

    /// Splits `source` into one node per case: the source with the case's constraints added.
    /// Returns the new nodes.
    pub fn split_on_constraints(
        &mut self,
        source: NodeId,
        cases: Vec<Vec<Pattern>>,
    ) -> R<Vec<NodeId>> {
        let base = self.node(source)?.cterm.clone();
        let targets: Vec<(NodeId, Vec<Pattern>)> = cases
            .into_iter()
            .map(|case| (self.create_node(base.with_constraints(case.clone())), case))
            .collect();
        let ids = targets.iter().map(|(t, _)| *t).collect();
        self.add_successor(source, Successor::Split { targets })?;
        Ok(ids)
    }

    pub fn create_ndbranch(
        &mut self,
        source: NodeId,
        cterms: Vec<CTerm>,
        rules: Vec<String>,
    ) -> R<Vec<NodeId>> {
        let targets: Vec<NodeId> = cterms.into_iter().map(|c| self.create_node(c)).collect();
        self.add_successor(
            source,
            Successor::NdBranch {
                targets: targets.clone(),
                rules,
            },
        )?;
        Ok(targets)
    }

    // --- queries ----------------------------------------------------------------------------

    pub fn is_leaf(&self, id: NodeId) -> bool {
        !self.successors.contains_key(&id)
    }

    pub fn is_covered(&self, id: NodeId) -> bool {
        matches!(self.successors.get(&id), Some(Successor::Cover { .. }))
    }

    /// Nodes without a successor.
    pub fn leaves(&self) -> Vec<NodeId> {
        self.nodes
            .keys()
            .copied()
            .filter(|id| self.is_leaf(*id))
            .collect()
    }

    /// Nodes without a predecessor.
    pub fn roots(&self) -> Vec<NodeId> {
        let targets: BTreeSet<NodeId> =
            self.successors.values().flat_map(|s| s.targets()).collect();
        self.nodes
            .keys()
            .copied()
            .filter(|id| !targets.contains(id))
            .collect()
    }

    /// Leaves that are not stuck, not vacuous, and not one of `except` (such as the target of a
    /// proof): where exploration goes on.
    pub fn frontier(&self, except: &[NodeId]) -> Vec<NodeId> {
        self.leaves()
            .into_iter()
            .filter(|id| !except.contains(id))
            .filter(|id| {
                let n = &self.nodes[id];
                !n.stuck && !n.vacuous
            })
            .collect()
    }

    pub fn stuck(&self) -> Vec<NodeId> {
        self.nodes
            .values()
            .filter(|n| n.stuck)
            .map(|n| n.id)
            .collect()
    }

    // --- aliases ----------------------------------------------------------------------------

    pub fn set_alias(&mut self, alias: impl Into<String>, id: NodeId) -> R<()> {
        self.node(id)?;
        self.aliases.insert(alias.into(), id);
        Ok(())
    }

    pub fn alias(&self, alias: &str) -> Option<NodeId> {
        self.aliases.get(alias).copied()
    }

    pub fn aliases(&self) -> &BTreeMap<String, NodeId> {
        &self.aliases
    }

    /// Whether anything changed since the graph was last saved or loaded.
    pub fn has_changes(&self) -> bool {
        !self.changed.is_empty() || !self.removed.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kore::text::parse_pattern;

    fn c(s: &str) -> CTerm {
        CTerm::new(parse_pattern(s).unwrap(), [])
    }

    #[test]
    fn build_and_prune() {
        let mut g = Kcfg::new();
        let a = g.create_node(c("a{}()"));
        let b = g.create_node(c("b{}()"));
        let t = g.create_node(c("t{}()"));
        g.create_edge(a, b, 3, vec![]).unwrap();
        assert!(g.create_edge(a, t, 1, vec![]).is_err(), "one successor");
        let cases = vec![
            vec![parse_pattern(r"\top{SortGeneratedTopCell{}}()").unwrap()],
            vec![parse_pattern("p{}()").unwrap()],
        ];
        let [b1, b2] = g.split_on_constraints(b, cases).unwrap()[..] else {
            panic!()
        };
        assert_eq!(g.node(b1).unwrap().cterm.constraints, vec![]);
        assert_eq!(g.node(b2).unwrap().cterm.constraints.len(), 1);
        g.create_cover(b1, t, CSubst::default()).unwrap();
        g.set_stuck(b2, true).unwrap();
        g.set_alias("target", t).unwrap();
        assert_eq!(g.roots(), vec![a], "the cover into t is a predecessor");
        assert_eq!(g.leaves(), vec![t, b2]);
        assert_eq!(g.frontier(&[t]), Vec::<NodeId>::new());
        assert!(g.is_covered(b1));

        // b's subtree goes; t stays, it is kept.
        let removed = g.prune(b, &[t]).unwrap();
        assert_eq!(removed.len(), 3);
        assert_eq!(g.len(), 2);
        assert!(g.is_leaf(a));
        assert_eq!(g.alias("target"), Some(t));
    }
}
