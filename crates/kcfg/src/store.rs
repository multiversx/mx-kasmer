//! A proof graph on disk, written as the proof goes, so that a proof stopped at any point
//! (finished, failed, killed) can be resumed or inspected.
//!
//! ```text
//! <dir>/kcfg.json        the structure: node ids, successors, aliases, stuck and vacuous nodes
//! <dir>/nodes/<id>.json  one node: {"id": ..., "cterm": {"config": ..., "constraints": [...]}}
//! ```
//!
//! The layout and field names are those of pyk's `KCFGStore`; patterns are KORE JSON (without
//! the format wrapper) where pyk has KAST.
//!
//! [`Store::save`] writes the nodes created or changed since the last save, then the structure,
//! then deletes the files of removed nodes. Every file is written to a temporary file that is
//! then renamed, so `kcfg.json` only ever names nodes whose files are complete.

use std::fs;
use std::io::BufWriter;
use std::path::{Path, PathBuf};

use kore::Pattern;
use serde::{Deserialize, Serialize};

use crate::cterm::{CSubst, CTerm};
use crate::graph::{Kcfg, Node, NodeId, Successor};

pub const INDEX_FILE: &str = "kcfg.json";
pub const NODES_DIR: &str = "nodes";

#[derive(Debug)]
pub enum StoreError {
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
    Format {
        path: PathBuf,
        message: String,
    },
}

impl std::fmt::Display for StoreError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StoreError::Io { path, source } => write!(f, "{}: {source}", path.display()),
            StoreError::Format { path, message } => write!(f, "{}: {message}", path.display()),
        }
    }
}

impl std::error::Error for StoreError {}

type R<T> = Result<T, StoreError>;

fn io(path: &Path) -> impl FnOnce(std::io::Error) -> StoreError {
    let path = path.to_path_buf();
    move |source| StoreError::Io { path, source }
}

pub struct Store {
    dir: PathBuf,
}

impl Store {
    /// A store in `dir`, created if needed.
    pub fn open(dir: impl Into<PathBuf>) -> R<Store> {
        let dir = dir.into();
        let nodes = dir.join(NODES_DIR);
        fs::create_dir_all(&nodes).map_err(io(&nodes))?;
        Ok(Store { dir })
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// Whether a graph was saved here.
    pub fn exists(dir: impl AsRef<Path>) -> bool {
        dir.as_ref().join(INDEX_FILE).is_file()
    }

    fn node_path(&self, id: NodeId) -> PathBuf {
        self.dir.join(NODES_DIR).join(format!("{id}.json"))
    }

    pub fn save(&self, kcfg: &mut Kcfg) -> R<()> {
        for id in &kcfg.changed {
            let node = &kcfg.nodes[id];
            write_atomic(&self.node_path(*id), |w| {
                kore::json::to_writer(
                    w,
                    &NodeFile {
                        id: node.id,
                        cterm: node.cterm.clone(),
                    },
                )
                .map_err(std::io::Error::other)
            })?;
        }
        let index = Index::of(kcfg);
        write_atomic(&self.dir.join(INDEX_FILE), |w| {
            kore::json::to_writer(w, &index).map_err(std::io::Error::other)
        })?;
        for id in &kcfg.removed {
            let path = self.node_path(*id);
            match fs::remove_file(&path) {
                Ok(()) => {}
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(e) => return Err(io(&path)(e)),
            }
        }
        kcfg.changed.clear();
        kcfg.removed.clear();
        Ok(())
    }

    pub fn load(&self) -> R<Kcfg> {
        let index: Index = read_json(&self.dir.join(INDEX_FILE))?;
        let mut kcfg = Kcfg {
            next: index.next,
            ..Default::default()
        };
        for id in &index.nodes {
            let path = self.node_path(*id);
            let file: NodeFile = read_json(&path)?;
            if file.id != *id {
                return Err(format(&path, format!("holds node {}", file.id)));
            }
            kcfg.nodes.insert(
                *id,
                Node {
                    id: *id,
                    cterm: file.cterm,
                    stuck: index.stuck.contains(id),
                    vacuous: index.vacuous.contains(id),
                },
            );
        }
        let index_path = self.dir.join(INDEX_FILE);
        let mut add = |source: NodeId, s: Successor| {
            kcfg.add_successor(source, s)
                .map_err(|e| format(&index_path, e.0))
        };
        for e in index.edges {
            add(
                e.source,
                Successor::Edge {
                    target: e.target,
                    depth: e.depth,
                    rules: e.rules,
                },
            )?;
        }
        for c in index.covers {
            add(
                c.source,
                Successor::Cover {
                    target: c.target,
                    csubst: c.csubst,
                },
            )?;
        }
        for s in index.splits {
            let targets = s
                .targets
                .into_iter()
                .map(|t| (t.target, t.constraints))
                .collect();
            add(s.source, Successor::Split { targets })?;
        }
        for b in index.ndbranches {
            add(
                b.source,
                Successor::NdBranch {
                    targets: b.targets,
                    rules: b.rules,
                },
            )?;
        }
        for (alias, id) in index.aliases {
            kcfg.set_alias(alias, id)
                .map_err(|e| format(&index_path, e.0))?;
        }
        if kcfg.nodes.keys().any(|id| *id >= kcfg.next) {
            return Err(format(
                &index_path,
                "a node id is not below \"next\"".into(),
            ));
        }
        Ok(kcfg)
    }
}

fn format(path: &Path, message: String) -> StoreError {
    StoreError::Format {
        path: path.to_path_buf(),
        message,
    }
}

fn read_json<T: for<'de> Deserialize<'de>>(path: &Path) -> R<T> {
    let text = fs::read_to_string(path).map_err(io(path))?;
    kore::json::from_str(&text).map_err(|e| format(path, e.to_string()))
}

/// Writes `path.tmp`, syncs it, renames it to `path`.
fn write_atomic(
    path: &Path,
    write: impl FnOnce(&mut BufWriter<fs::File>) -> std::io::Result<()>,
) -> R<()> {
    let tmp = path.with_extension("json.tmp");
    let file = fs::File::create(&tmp).map_err(io(&tmp))?;
    let mut w = BufWriter::new(file);
    write(&mut w).map_err(io(&tmp))?;
    let file = w.into_inner().map_err(|e| io(&tmp)(e.into_error()))?;
    file.sync_all().map_err(io(&tmp))?;
    fs::rename(&tmp, path).map_err(io(path))
}

#[derive(Serialize, Deserialize)]
struct NodeFile {
    id: NodeId,
    cterm: CTerm,
}

#[derive(Serialize, Deserialize)]
struct Index {
    next: NodeId,
    nodes: Vec<NodeId>,
    #[serde(default)]
    edges: Vec<EdgeEntry>,
    #[serde(default)]
    covers: Vec<CoverEntry>,
    #[serde(default)]
    splits: Vec<SplitEntry>,
    #[serde(default)]
    ndbranches: Vec<NdBranchEntry>,
    #[serde(default)]
    aliases: std::collections::BTreeMap<String, NodeId>,
    #[serde(default)]
    stuck: Vec<NodeId>,
    #[serde(default)]
    vacuous: Vec<NodeId>,
}

#[derive(Serialize, Deserialize)]
struct EdgeEntry {
    source: NodeId,
    target: NodeId,
    depth: u64,
    rules: Vec<String>,
}

#[derive(Serialize, Deserialize)]
struct CoverEntry {
    source: NodeId,
    target: NodeId,
    csubst: CSubst,
}

#[derive(Serialize, Deserialize)]
struct SplitEntry {
    source: NodeId,
    targets: Vec<SplitTarget>,
}

#[derive(Serialize, Deserialize)]
struct SplitTarget {
    target: NodeId,
    constraints: Vec<Pattern>,
}

#[derive(Serialize, Deserialize)]
struct NdBranchEntry {
    source: NodeId,
    targets: Vec<NodeId>,
    rules: Vec<String>,
}

impl Index {
    fn of(kcfg: &Kcfg) -> Index {
        let mut index = Index {
            next: kcfg.next,
            nodes: kcfg.nodes.keys().copied().collect(),
            edges: vec![],
            covers: vec![],
            splits: vec![],
            ndbranches: vec![],
            aliases: kcfg.aliases.clone(),
            stuck: kcfg.stuck(),
            vacuous: kcfg
                .nodes
                .values()
                .filter(|n| n.vacuous)
                .map(|n| n.id)
                .collect(),
        };
        for (source, s) in &kcfg.successors {
            let source = *source;
            match s.clone() {
                Successor::Edge {
                    target,
                    depth,
                    rules,
                } => index.edges.push(EdgeEntry {
                    source,
                    target,
                    depth,
                    rules,
                }),
                Successor::Cover { target, csubst } => index.covers.push(CoverEntry {
                    source,
                    target,
                    csubst,
                }),
                Successor::Split { targets } => index.splits.push(SplitEntry {
                    source,
                    targets: targets
                        .into_iter()
                        .map(|(target, constraints)| SplitTarget {
                            target,
                            constraints,
                        })
                        .collect(),
                }),
                Successor::NdBranch { targets, rules } => index.ndbranches.push(NdBranchEntry {
                    source,
                    targets,
                    rules,
                }),
            }
        }
        index
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kore::text::parse_pattern;

    fn p(s: &str) -> Pattern {
        parse_pattern(s).unwrap()
    }

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("kcfg-store-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        dir
    }

    #[test]
    fn save_load_resume() {
        let dir = scratch("resume");
        let store = Store::open(&dir).unwrap();
        let mut g = Kcfg::new();
        let a = g.create_node(CTerm::new(p("a{}()"), [p("c1{}()")]));
        let t = g.create_node(CTerm::new(p("t{}()"), []));
        g.set_alias("init", a).unwrap();
        g.set_alias("target", t).unwrap();
        let [x, y] = g
            .split_on_constraints(a, vec![vec![p("x{}()")], vec![p("y{}()")]])
            .unwrap()[..]
        else {
            panic!()
        };
        g.create_cover(
            x,
            t,
            CSubst {
                substitution: Some(p("s{}()")),
                constraints: vec![],
            },
        )
        .unwrap();
        g.set_stuck(y, true).unwrap();
        store.save(&mut g).unwrap();
        assert!(!g.has_changes());

        // Resumed: same graph, new ids continue.
        let mut h = store.load().unwrap();
        assert_eq!(h.len(), 4);
        assert_eq!(h.successor(a), g.successor(a));
        assert_eq!(h.successor(x), g.successor(x));
        assert_eq!(h.node(x).unwrap(), g.node(x).unwrap());
        assert_eq!(h.stuck(), vec![y]);
        assert_eq!(h.alias("target"), Some(t));
        let z = h.create_node(CTerm::new(p("z{}()"), []));
        assert_eq!(z, 5);

        // Removing a node deletes its file at the next save.
        h.remove_node(y).unwrap();
        store.save(&mut h).unwrap();
        assert!(!store.node_path(y).exists());
        assert!(store.node_path(z).exists());
        let k = store.load().unwrap();
        assert_eq!(k.len(), 4);
        assert!(k.successor(a).is_none(), "the split lost a target and went");
        let _ = fs::remove_dir_all(&dir);
    }
}
