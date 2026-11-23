//! Checks the client against proofs of the Python tooling, recorded by `scripts/rpc-corpus.sh`
//! into `.conformance/rpc/<claim>/`: every JSON-RPC exchange pyk had with `kore-rpc-booster`.
//!
//! - `messages_round_trip`: every request and result reads into the types of
//!   [`kore_rpc::protocol`] and writes back to the same JSON.
//! - `replay`: the recorded requests, sent by this client to a fresh server, get the recorded
//!   results. Needs a server directory in `KASMER_RPC_SERVER` (`scripts/kore-rpc-host.sh`);
//!   skipped otherwise. Takes about as long as the proof did.
//!
//! Skipped when there is no reference data.

use std::fs;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

use kore_rpc::protocol::*;
use kore_rpc::server::{self, Server};
use serde::Serialize;
use serde::de::DeserializeOwned;
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

struct Exchange {
    method: String,
    params: Value,
    result: Value,
}

fn claims(corpus: &Path) -> Vec<PathBuf> {
    let mut dirs: Vec<_> = fs::read_dir(corpus)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.join("exchanges.jsonl").is_file())
        .collect();
    dirs.sort();
    dirs
}

fn exchanges(claim: &Path) -> impl Iterator<Item = Exchange> {
    let file = fs::File::open(claim.join("exchanges.jsonl")).unwrap();
    BufReader::new(file).lines().map(|line| {
        let mut e: Value = kore::json::from_str(&line.unwrap()).unwrap();
        let request = e["request"].take();
        Exchange {
            method: request["method"].as_str().unwrap().to_owned(),
            params: request["params"].clone(),
            result: e["response"]["result"].take(),
        }
    })
}

/// Reads `value` as `T` and writes it back.
fn round_trip<T: Serialize + DeserializeOwned>(value: &Value) -> Value {
    let text = value.to_string();
    let typed: T = kore::json::from_str(&text).unwrap();
    serde_json::to_value(&typed).unwrap()
}

fn check<T: Serialize + DeserializeOwned>(what: &str, value: &Value) {
    let back = round_trip::<T>(value);
    if &back != value {
        panic!(
            "{what}: differs after reading and writing:\n{}",
            diff(value, &back)
        );
    }
}

/// The first place where two JSON values differ.
fn diff(a: &Value, b: &Value) -> String {
    fn go(path: &mut String, a: &Value, b: &Value) -> Option<String> {
        match (a, b) {
            (Value::Object(x), Value::Object(y)) => {
                for k in x.keys().chain(y.keys()) {
                    let len = path.len();
                    path.push('.');
                    path.push_str(k);
                    let r = match (x.get(k), y.get(k)) {
                        (Some(u), Some(v)) => go(path, u, v),
                        (u, v) => Some(format!("{path}: {u:?} / {v:?}")),
                    };
                    if r.is_some() {
                        return r;
                    }
                    path.truncate(len);
                }
                None
            }
            (Value::Array(x), Value::Array(y)) if x.len() == y.len() => {
                for (i, (u, v)) in x.iter().zip(y).enumerate() {
                    let len = path.len();
                    path.push_str(&format!("[{i}]"));
                    if let Some(r) = go(path, u, v) {
                        return Some(r);
                    }
                    path.truncate(len);
                }
                None
            }
            _ if a == b => None,
            _ => {
                let (u, v) = (a.to_string(), b.to_string());
                Some(format!(
                    "{path}:\n  expected {}\n  got      {}",
                    &u[..u.floor_char_boundary(300)],
                    &v[..v.floor_char_boundary(300)]
                ))
            }
        }
    }
    go(&mut String::new(), a, b).unwrap_or_else(|| "(equal)".into())
}

#[test]
fn messages_round_trip() {
    let Some(corpus) = corpus() else { return };
    let mut counts = std::collections::BTreeMap::<String, usize>::new();
    for claim in claims(&corpus) {
        for (i, e) in exchanges(&claim).enumerate() {
            let what = format!("{} #{i} {}", claim.display(), e.method);
            match e.method.as_str() {
                "execute" => {
                    check::<ExecuteParams>(&what, &e.params);
                    check::<ExecuteResult>(&what, &e.result);
                }
                "implies" => {
                    check::<ImpliesParams>(&what, &e.params);
                    check::<ImpliesResult>(&what, &e.result);
                }
                "simplify" => {
                    check::<SimplifyParams>(&what, &e.params);
                    check::<SimplifyResult>(&what, &e.result);
                }
                "get-model" => {
                    check::<GetModelParams>(&what, &e.params);
                    check::<GetModelResult>(&what, &e.result);
                }
                "add-module" => {
                    check::<AddModuleParams>(&what, &e.params);
                    check::<AddModuleResult>(&what, &e.result);
                }
                m => panic!("{what}: method {m} not covered"),
            }
            *counts.entry(e.method).or_default() += 1;
        }
    }
    eprintln!("round trip: {counts:?}");
}

#[test]
fn replay() {
    let Some(corpus) = corpus() else { return };
    let Some(dir) = std::env::var_os("KASMER_RPC_SERVER") else {
        eprintln!("KASMER_RPC_SERVER not set: replay skipped");
        return;
    };
    for claim in claims(&corpus) {
        let recorded: Value =
            serde_json::from_str(&fs::read_to_string(claim.join("server.json")).unwrap()).unwrap();
        let args: Vec<String> = recorded["options"]
            .as_array()
            .unwrap()
            .iter()
            .map(|a| a.as_str().unwrap().to_owned())
            .collect();
        let definition = Path::new(&dir).join(server::DEFINITION_FILE);
        assert!(definition.is_file(), "{}", definition.display());
        let server = Server::start(
            &dir,
            &server::Options {
                args,
                ..Default::default()
            },
        )
        .unwrap();
        let mut client = server.connect().unwrap();
        let start = std::time::Instant::now();
        for (i, e) in exchanges(&claim).enumerate() {
            let what = format!("{} #{i} {}", claim.display(), e.method);
            let params: Value = match e.method.as_str() {
                // Typed, as the client sends them.
                "execute" => serde_json::to_value(round_trip::<ExecuteParams>(&e.params)).unwrap(),
                "implies" => serde_json::to_value(round_trip::<ImpliesParams>(&e.params)).unwrap(),
                _ => e.params,
            };
            let result = client
                .request_value(&e.method, &params)
                .unwrap_or_else(|err| panic!("{what}: {err}{}", server.tail_text()));
            if result != e.result {
                panic!("{what}: another result:\n{}", diff(&e.result, &result));
            }
        }
        eprintln!("{}: replayed in {:.1?}", claim.display(), start.elapsed());
    }
}
