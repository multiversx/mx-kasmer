//! Checks the symbolic path against `kasmer verify` of the Python tooling, on the proofs recorded
//! by `scripts/rpc-corpus.sh` in `.conformance/rpc/<project>.<endpoint>/`.
//!
//! - `claims_like_pyk`: the first two nodes of the proof graph (`init.kore`, `target.kore`, as pyk
//!   sends them to the server). The configurations must be the same byte for byte; the
//!   constraints the same set (pyk orders them by the length of their KAST text). Needs
//!   `KASMER_LLVM_DEFINITION` to deploy.
//! - `proofs_like_pyk`: the whole proof, by the Rust prover, against a fresh server: the same
//!   result, and the same graph as pyk's (`kcfg/kcfg.json`): node ids, edges with their depths,
//!   splits, branches, covers, stuck nodes. Also needs `KASMER_RPC_SERVER`; takes minutes.
//!
//! Skipped without the reference data or the variables.

use std::fs;
use std::path::{Path, PathBuf};

use kasmer::LlvmDefinition;
use kasmer::claim::{self, Claim};
use kasmer::project::Project;
use kasmer::setup::{self, Contract};
use kasmer::steps::Builder;
use kcfg::CTerm;
use kcfg::cterm::conjuncts;

/// Where the repository is in the container that recorded the reference data: pyk names the
/// contracts by their absolute paths.
const CONTAINER_ROOT: &str = "/home/user/mx-kasmer";

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap()
}

/// A contract named by its path in the container.
fn contract(path: &Path) -> Contract {
    let relative = path.strip_prefix(root()).unwrap();
    Contract {
        name: format!("{CONTAINER_ROOT}/{}", relative.display()),
        wasm: fs::read(path).unwrap(),
    }
}

fn cterm_of(text: &str) -> CTerm {
    let p = kore::text::parse_pattern(text.trim()).unwrap();
    let mut parts = conjuncts(p);
    let config = parts.remove(0);
    CTerm::new(config, parts)
}

/// Every recorded proof: its directory, name and the claim built here, deployed with the
/// contracts named as pyk named them.
fn recorded_claims(definition: &LlvmDefinition) -> Vec<(PathBuf, String, Claim)> {
    let corpus = root().join(".conformance/rpc");
    let b = Builder::new(definition.symbols());
    // The recorder loads the ESDT system contract by its absolute path too.
    let esdt = Contract {
        name: format!("{CONTAINER_ROOT}/contracts/esdt-system-sc/output/esdt-system-sc.wasm"),
        ..Contract::esdt_system_sc()
    };
    let runtime = setup::runtime_config(definition, &esdt).unwrap();
    let mut proofs: Vec<PathBuf> = fs::read_dir(&corpus)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.join("init.kore").is_file())
        .collect();
    proofs.sort();
    proofs
        .into_iter()
        .map(|proof| {
            let name = proof.file_name().unwrap().to_str().unwrap().to_owned();
            let (project_name, endpoint_name) = name.split_once('.').unwrap();
            let project =
                Project::load(root().join("contracts/parametric-tests").join(project_name))
                    .unwrap();
            let contracts: Vec<Contract> = project.contracts.iter().map(|c| contract(c)).collect();
            let deployed = setup::deploy(
                definition,
                runtime.clone(),
                &contract(&project.test_wasm),
                &contracts,
            )
            .unwrap();
            let endpoint = project
                .endpoints
                .iter()
                .find(|e| e.name == endpoint_name)
                .unwrap();
            let claim = claim::build(&b, &deployed, endpoint).unwrap();
            (proof, name, claim)
        })
        .collect()
}

fn setup() -> Option<LlvmDefinition> {
    let corpus = root().join(".conformance/rpc");
    if !corpus.is_dir() {
        eprintln!(
            "no reference data in {}: run scripts/rpc-corpus.sh",
            corpus.display()
        );
        return None;
    }
    let Some(dir) = std::env::var_os("KASMER_LLVM_DEFINITION") else {
        eprintln!("KASMER_LLVM_DEFINITION not set: skipped");
        return None;
    };
    Some(LlvmDefinition::load(dir).unwrap())
}

#[test]
fn claims_like_pyk() {
    let Some(definition) = setup() else { return };
    for (proof, name, claim) in recorded_claims(&definition) {
        for (which, ours) in [("init", &claim.init), ("target", &claim.target)] {
            let pyk = cterm_of(&fs::read_to_string(proof.join(format!("{which}.kore"))).unwrap());
            let (a, e) = (ours.config.to_string(), pyk.config.to_string());
            if a != e {
                let at = a.bytes().zip(e.bytes()).take_while(|(x, y)| x == y).count();
                let ctx = |s: &str| {
                    let start = s.floor_char_boundary(at.saturating_sub(200));
                    let end = s.ceil_char_boundary((at + 200).min(s.len()));
                    s[start..end].to_owned()
                };
                panic!(
                    "{name} {which}: configurations differ at byte {at}:\n  ours: {}\n  pyk:  {}",
                    ctx(&a),
                    ctx(&e)
                );
            }
            let mut ours_c: Vec<String> = ours.constraints.iter().map(|c| c.to_string()).collect();
            let mut pyk_c: Vec<String> = pyk.constraints.iter().map(|c| c.to_string()).collect();
            ours_c.sort();
            pyk_c.sort();
            assert_eq!(ours_c, pyk_c, "{name} {which}: constraints");
        }
        eprintln!("{name}: init and target as pyk's");
    }
}

/// The structure of a graph, as pyk's kcfg.json has it, without terms.
fn shape_of_pyk(kcfg_json: &Path) -> serde_json::Value {
    let d: serde_json::Value =
        kore::json::from_str(&fs::read_to_string(kcfg_json).unwrap()).unwrap();
    let list = |key: &str| {
        d.get(key)
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default()
    };
    serde_json::json!({
        "nodes": d["nodes"],
        "edges": list("edges").iter().map(|e| (e["source"].clone(), e["target"].clone(), e["depth"].clone())).collect::<Vec<_>>(),
        "covers": list("covers").iter().map(|e| (e["source"].clone(), e["target"].clone())).collect::<Vec<_>>(),
        "splits": list("splits").iter().map(|e| (e["source"].clone(), e["targets"].as_array().unwrap().iter().map(|t| t["target"].clone()).collect::<Vec<_>>())).collect::<Vec<_>>(),
        "ndbranches": list("ndbranches").iter().map(|e| (e["source"].clone(), e["targets"].clone())).collect::<Vec<_>>(),
        "stuck": list("stuck"),
    })
}

fn shape_of(kcfg: &kcfg::Kcfg) -> serde_json::Value {
    use kcfg::Successor;
    let (mut edges, mut covers, mut splits, mut nds) = (vec![], vec![], vec![], vec![]);
    for (source, s) in kcfg.successors() {
        match s {
            Successor::Edge { target, depth, .. } => {
                edges.push(serde_json::json!([source, target, depth]))
            }
            Successor::Cover { target, .. } => covers.push(serde_json::json!([source, target])),
            Successor::Split { targets } => splits.push(serde_json::json!([
                source,
                targets.iter().map(|t| t.0).collect::<Vec<_>>()
            ])),
            Successor::NdBranch { targets, .. } => nds.push(serde_json::json!([source, targets])),
        }
    }
    serde_json::json!({
        "nodes": kcfg.nodes().map(|n| n.id).collect::<Vec<_>>(),
        "edges": edges, "covers": covers, "splits": splits, "ndbranches": nds,
        "stuck": kcfg.stuck(),
    })
}

#[test]
fn proofs_like_pyk() {
    use kasmer::prove::{self, Prover, Status};
    use kore_rpc::server::{self, Server};

    let Some(definition) = setup() else { return };
    let Some(server_dir) = std::env::var_os("KASMER_RPC_SERVER") else {
        eprintln!("KASMER_RPC_SERVER not set: skipped");
        return;
    };
    let text = fs::read_to_string(Path::new(&server_dir).join("definition.kore")).unwrap();
    let labels = kcfg::RuleLabels::from_definition(&kore::text::parse_definition(&text).unwrap());
    let server = Server::start(
        &server_dir,
        &server::Options {
            args: prove::BOOSTER_ARGS.iter().map(|a| a.to_string()).collect(),
            ..Default::default()
        },
    )
    .unwrap();
    let mut client = server.connect().unwrap();
    let scratch = std::env::temp_dir().join(format!("kasmer-proofs-{}", std::process::id()));
    for (proof, name, claim) in recorded_claims(&definition) {
        let start = std::time::Instant::now();
        let mut prover = Prover {
            definition: &definition,
            client: &mut client,
            labels: &labels,
            options: prove::Options {
                fresh: true,
                ..Default::default()
            },
        };
        let outcome = prover
            .prove(&claim, &scratch.join(&name), &mut |_| {})
            .unwrap();
        let pyk_result = fs::read_to_string(proof.join("result.txt")).unwrap();
        let expected = match pyk_result.trim() {
            "Success" => matches!(outcome.status, Status::Proved),
            "Failed" => matches!(outcome.status, Status::Failed { .. }),
            other => panic!("{name}: pyk's run ended with {other}"),
        };
        assert!(
            expected,
            "{name}: pyk {}, ours {:?}",
            pyk_result.trim(),
            outcome.status
        );
        assert_eq!(
            shape_of(&outcome.kcfg),
            shape_of_pyk(&proof.join("kcfg/kcfg.json")),
            "{name}: the graph"
        );
        eprintln!(
            "{name}: {:?} with the graph of pyk ({} nodes) in {:.1?}",
            outcome.status,
            outcome.kcfg.len(),
            start.elapsed()
        );
    }
    let _ = fs::remove_dir_all(&scratch);
}
