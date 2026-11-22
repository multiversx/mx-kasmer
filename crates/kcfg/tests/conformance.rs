//! Checks the graph against proofs of the Python tooling, recorded by `scripts/rpc-corpus.sh`
//! into `.conformance/rpc/<claim>/`.
//!
//! - `classify_like_pyk`: every `execute` result of the proof is read into the same extensions
//!   as pyk's `extend_cterm` read it (`extends.jsonl`): kinds, depths, cut points, number of
//!   branches, and the names of the rules on each step. The names need the definition, from the
//!   server directory in `KASMER_RPC_SERVER`; without it only their number is compared.
//! - `store_real_nodes`: the first extension of every result, added to a graph and saved, loads
//!   back the same.
//!
//! Skipped when there is no reference data.

use std::fs;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

use kcfg::{CTerm, Extension, Kcfg, RuleLabels, Store, apply, classify};
use kore_rpc::protocol::ExecuteResult;
use serde_json::Value;

fn corpus() -> Option<PathBuf> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../.conformance/rpc");
    if dir.is_dir() {
        Some(dir)
    } else {
        eprintln!(
            "no reference data in {}: run scripts/rpc-corpus.sh",
            dir.display()
        );
        None
    }
}

fn claims(corpus: &Path) -> Vec<PathBuf> {
    let mut dirs: Vec<_> = fs::read_dir(corpus)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.join("extends.jsonl").is_file())
        .collect();
    dirs.sort();
    dirs
}

fn lines(path: &Path) -> impl Iterator<Item = String> {
    BufReader::new(fs::File::open(path).unwrap())
        .lines()
        .map(Result::unwrap)
}

/// The `execute` exchanges of a claim: request state and result.
fn executes(claim: &Path) -> Vec<Option<ExecuteResult>> {
    lines(&claim.join("exchanges.jsonl"))
        .map(|line| {
            let mut e: Value = kore::json::from_str(&line).unwrap();
            if e["request"]["method"] != "execute" {
                return None;
            }
            let text = e["response"]["result"].take().to_string();
            Some(kore::json::from_str(&text).unwrap())
        })
        .collect()
}

fn labels() -> Option<RuleLabels> {
    let dir = std::env::var_os("KASMER_RPC_SERVER")?;
    let path = Path::new(&dir).join("definition.kore");
    let text = fs::read_to_string(&path).unwrap();
    let definition = kore::text::parse_definition(&text).unwrap();
    Some(RuleLabels::from_definition(&definition))
}

/// An extension in the form of `extends.jsonl`.
fn describe(e: &Extension, with_rules: bool) -> Value {
    let rules = |rules: &[String]| {
        if with_rules {
            serde_json::json!(rules)
        } else {
            serde_json::json!(rules.len())
        }
    };
    match e {
        Extension::Step {
            depth,
            cut,
            rules: r,
            ..
        } => serde_json::json!({"kind": "step", "depth": depth, "cut": cut, "rules": rules(r)}),
        Extension::Stuck => serde_json::json!({"kind": "stuck"}),
        Extension::Vacuous => serde_json::json!({"kind": "vacuous"}),
        Extension::Branch { cases } => {
            serde_json::json!({"kind": "branch", "branches": cases.len()})
        }
        Extension::NdBranch { cterms, rules: r } => {
            serde_json::json!({"kind": "ndbranch", "branches": cterms.len(), "rules": rules(r)})
        }
    }
}

fn pyk_described(v: &Value, with_rules: bool) -> Value {
    let mut v = v.clone();
    if !with_rules && let Some(r) = v.get("rules") {
        v["rules"] = serde_json::json!(r.as_array().unwrap().len());
    }
    v
}

#[test]
fn classify_like_pyk() {
    let Some(corpus) = corpus() else { return };
    let labels = labels();
    if labels.is_none() {
        eprintln!("KASMER_RPC_SERVER not set: rule names not compared, only their number");
    }
    let with_rules = labels.is_some();
    let labels = labels.unwrap_or_default();
    for claim in claims(&corpus) {
        let executes = executes(&claim);
        let mut checked = 0;
        for line in lines(&claim.join("extends.jsonl")) {
            let recorded: Value = serde_json::from_str(&line).unwrap();
            let Some(i) = recorded["exchange"].as_u64() else {
                continue;
            };
            let result = executes[i as usize].as_ref().expect("an execute exchange");
            let ours: Vec<Value> = classify(result, &labels)
                .unwrap()
                .iter()
                .map(|e| describe(e, with_rules))
                .collect();
            let pyk: Vec<Value> = recorded["results"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| pyk_described(v, with_rules))
                .collect();
            assert_eq!(ours, pyk, "{} exchange {i}", claim.display());
            checked += 1;
        }
        eprintln!(
            "{}: {checked} results classified as pyk did",
            claim.display()
        );
    }
}

#[test]
fn store_real_nodes() {
    let Some(corpus) = corpus() else { return };
    let dir = std::env::temp_dir().join(format!("kcfg-conformance-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    for claim in claims(&corpus) {
        let store = Store::open(dir.join(claim.file_name().unwrap())).unwrap();
        let mut g = Kcfg::new();
        for result in executes(&claim).into_iter().flatten() {
            let node = g.create_node(CTerm::from_state(&result.state));
            let first = classify(&result, &RuleLabels::none())
                .unwrap()
                .into_iter()
                .next();
            if let Some(e) = first {
                apply(&mut g, node, e).unwrap();
            }
        }
        let start = std::time::Instant::now();
        store.save(&mut g).unwrap();
        let saved = start.elapsed();
        let start = std::time::Instant::now();
        let h = store.load().unwrap();
        eprintln!(
            "{}: {} nodes saved in {saved:.1?}, loaded in {:.1?}",
            claim.display(),
            g.len(),
            start.elapsed()
        );
        assert_eq!(h.len(), g.len());
        for n in g.nodes() {
            assert_eq!(h.node(n.id).unwrap(), n);
            assert_eq!(h.successor(n.id), g.successor(n.id));
        }
    }
    let _ = fs::remove_dir_all(&dir);
}

/// A recorded pattern `\and(config, constraints...)` (or `\exists`s around one) as a CTerm.
fn cterm_of(p: &Value) -> CTerm {
    let mut p: kore::Pattern = kore::json::parse_term(&p.to_string()).unwrap();
    while let kore::Pattern::Ml(kore::Ml {
        conn: kore::Conn::Exists,
        args,
        ..
    }) = p
    {
        p = args.into_vec().pop().unwrap();
    }
    let mut parts = kcfg::cterm::conjuncts(p);
    let config = parts.remove(0);
    CTerm::new(config, parts)
}

/// With a live server (`KASMER_RPC_SERVER`): the first node of the recorded proof extended by
/// this crate as pyk extended it; the graph saved, loaded again (a resumed proof) and extended
/// further; the recorded implication checks decided as pyk's were.
#[test]
fn live_extend_and_resume() {
    use kcfg::explore::{extend_node, implies};
    use kore_rpc::protocol::ExecuteParams;
    use kore_rpc::server::{Options, Server};

    let Some(corpus) = corpus() else { return };
    let Some(dir) = std::env::var_os("KASMER_RPC_SERVER") else {
        eprintln!("KASMER_RPC_SERVER not set: skipped");
        return;
    };
    let labels = labels().unwrap();
    for claim in claims(&corpus) {
        let server_json: Value =
            serde_json::from_str(&fs::read_to_string(claim.join("server.json")).unwrap()).unwrap();
        let args = server_json["options"]
            .as_array()
            .unwrap()
            .iter()
            .map(|a| a.as_str().unwrap().to_owned())
            .collect();
        let server = Server::start(
            &dir,
            &Options {
                args,
                ..Default::default()
            },
        )
        .unwrap();
        let mut client = server.connect().unwrap();

        let exchanges: Vec<Value> = lines(&claim.join("exchanges.jsonl"))
            .map(|l| kore::json::from_str(&l).unwrap())
            .collect();
        let first = &exchanges[0]["request"]["params"];
        let params: ExecuteParams = kore::json::from_str(&first.to_string()).unwrap();
        let recorded: Value =
            serde_json::from_str(&lines(&claim.join("extends.jsonl")).next().unwrap()).unwrap();
        assert_eq!(recorded["exchange"], 0);

        let store_dir = std::env::temp_dir().join(format!("kcfg-live-{}", std::process::id()));
        let _ = fs::remove_dir_all(&store_dir);
        let store = Store::open(&store_dir).unwrap();
        let mut g = Kcfg::new();
        let init = g.create_node(cterm_of(&first["state"]));
        g.set_alias("init", init).unwrap();
        let (extensions, new) = extend_node(&mut client, &mut g, init, &params, &labels).unwrap();
        let ours: Vec<Value> = extensions.iter().map(|e| describe(e, true)).collect();
        assert_eq!(&ours, recorded["results"].as_array().unwrap());
        store.save(&mut g).unwrap();
        drop(g);

        // Resumed from disk: the leaf is where the proof goes on.
        let mut g = store.load().unwrap();
        assert_eq!(g.frontier(&[]), new);
        let (extensions, _) = extend_node(&mut client, &mut g, new[0], &params, &labels).unwrap();
        assert!(!extensions.is_empty());
        store.save(&mut g).unwrap();
        assert_eq!(store.load().unwrap().len(), g.len());
        eprintln!(
            "{}: extended, resumed, extended: {} nodes",
            claim.display(),
            g.len()
        );

        for (i, e) in exchanges.iter().enumerate() {
            if e["request"]["method"] != "implies" {
                continue;
            }
            let p = &e["request"]["params"];
            let valid = e["response"]["result"]["status"] == "valid";
            let ours = implies(
                &mut client,
                &cterm_of(&p["antecedent"]),
                &cterm_of(&p["consequent"]),
            )
            .unwrap();
            assert_eq!(ours.is_some(), valid, "implies, exchange {i}");
            eprintln!("{}: implies #{i}: valid {valid}, as pyk", claim.display());
        }
        let _ = fs::remove_dir_all(&store_dir);
    }
}
