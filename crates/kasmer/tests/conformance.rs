//! Checks the test setup against `kasmer fuzz` of the Python tooling, on the test projects in
//! `contracts/parametric-tests`. Reference data from `scripts/kore-corpus.sh` in
//! `.conformance/kore/` (or set `KORE_CORPUS`); without it the tests pass with a notice on stderr.
//!
//! - `call_steps`: the steps of a test call, byte for byte. Needs only `definition.kore`.
//! - `runtime`, `templates`, `exit_codes`: the runtime configuration, the deployed template and
//!   the outcome of each call. These run the interpreter, and need a definition directory in
//!   `KASMER_LLVM_DEFINITION` (`scripts/llvm-kasmer-host.sh` writes one); without it they pass
//!   with a notice.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::Instant;

use kasmer::fuzz::{Template, call_steps};
use kasmer::project::Project;
use kasmer::setup::{self, Contract};
use kasmer::steps::Builder;
use kasmer::{LlvmDefinition, mandos};
use kore::symbols::Symbols;
use serde_json::Value;

const PROJECTS: [&str; 4] = [
    "adder-pt",
    "adder-pt-fail",
    "crowdfunding-pt",
    "multisig-pt",
];

/// The name of the ESDT system contract in the reference runtime configuration: its path.
const ESDT_NAME: &str = "contracts/esdt-system-sc/output/esdt-system-sc.wasm";

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap()
}

fn corpus() -> Option<PathBuf> {
    let dir = match std::env::var_os("KORE_CORPUS") {
        Some(d) => PathBuf::from(d),
        None => root().join(".conformance/kore"),
    };
    if dir.join("kasmer").is_dir() {
        Some(dir)
    } else {
        eprintln!(
            "no kasmer reference data in {}; run scripts/kore-corpus.sh",
            dir.display()
        );
        None
    }
}

fn definition() -> Option<LlvmDefinition> {
    let Some(dir) = std::env::var_os("KASMER_LLVM_DEFINITION") else {
        eprintln!(
            "KASMER_LLVM_DEFINITION not set; not running the interpreter (scripts/llvm-kasmer-host.sh)"
        );
        return None;
    };
    Some(LlvmDefinition::load(dir).unwrap())
}

fn read(path: &Path) -> String {
    fs::read_to_string(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

fn first_difference(actual: &str, expected: &str) -> String {
    let at = actual
        .bytes()
        .zip(expected.bytes())
        .take_while(|(a, b)| a == b)
        .count();
    let context = |s: &str| {
        let start = s.floor_char_boundary(at.saturating_sub(150));
        let end = s.ceil_char_boundary((at + 150).min(s.len()));
        s[start..end].to_owned()
    };
    format!(
        "differs at byte {at} (lengths {} and {})\n  actual:   {}\n  expected: {}",
        actual.len(),
        expected.len(),
        context(actual),
        context(expected)
    )
}

struct Call {
    endpoint: String,
    args: Vec<Vec<u8>>,
    steps: String,
    exit_code: i32,
}

fn calls(corpus: &Path, project: &str) -> Vec<Call> {
    read(&corpus.join("kasmer").join(project).join("calls.jsonl"))
        .lines()
        .map(|line| {
            let v: Value = serde_json::from_str(line).unwrap();
            Call {
                endpoint: v["endpoint"].as_str().unwrap().to_owned(),
                args: v["args"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|a| mandos::from_decimal(a.as_str().unwrap()).unwrap())
                    .collect(),
                steps: v["steps"].as_str().unwrap().to_owned(),
                exit_code: v["exit_code"].as_i64().unwrap() as i32,
            }
        })
        .collect()
}

#[test]
fn call_steps_match() {
    let Some(corpus) = corpus() else { return };
    let text = read(&corpus.join("definitions/mx-semantics-llvm-kasmer.kore"));
    let symbols = Symbols::new(&kore::text::parse_definition(&text).unwrap());
    let builder = Builder::new(&symbols);
    let mut n = 0;
    for project in PROJECTS {
        for call in calls(&corpus, project) {
            let steps = call_steps(&builder, &call.endpoint, &call.args)
                .unwrap()
                .to_string();
            if steps != call.steps {
                panic!(
                    "{project} {}: {}",
                    call.endpoint,
                    first_difference(&steps, &call.steps)
                );
            }
            n += 1;
        }
    }
    assert!(n > 0);
    eprintln!("{n} calls");
}

/// The contracts of a project, named by their paths relative to the repository root, as the
/// reference data names them.
fn contracts(project: &str) -> (Contract, Vec<Contract>) {
    let root = root();
    let p = Project::load(root.join("contracts/parametric-tests").join(project)).unwrap();
    let contract = |path: &Path| {
        let mut c = Contract::read(path).unwrap();
        c.name = path.strip_prefix(&root).unwrap().display().to_string();
        c
    };
    (
        contract(&p.test_wasm),
        p.contracts.iter().map(|c| contract(c)).collect(),
    )
}

#[test]
fn runtime() {
    let (Some(corpus), Some(definition)) = (corpus(), definition()) else {
        return;
    };
    let esdt = Contract {
        name: ESDT_NAME.into(),
        ..Contract::esdt_system_sc()
    };
    let t = Instant::now();
    let runtime = setup::runtime_config(&definition, &esdt)
        .unwrap()
        .to_string();
    eprintln!("runtime configuration in {:?}", t.elapsed());
    let expected = read(&corpus.join("kasmer/runtime.kore"));
    if runtime != expected {
        panic!("runtime: {}", first_difference(&runtime, &expected));
    }
}

fn templates(definition: &LlvmDefinition) -> Vec<(&'static str, Template)> {
    let esdt = Contract {
        name: ESDT_NAME.into(),
        ..Contract::esdt_system_sc()
    };
    let runtime = setup::runtime_config(definition, &esdt).unwrap();
    PROJECTS
        .iter()
        .map(|&project| {
            let (test, contracts) = contracts(project);
            let t = Instant::now();
            let deployed = setup::deploy(definition, runtime.clone(), &test, &contracts).unwrap();
            eprintln!("{project}: deployed in {:?}", t.elapsed());
            (project, Template::new(&deployed).unwrap())
        })
        .collect()
}

#[test]
fn templates_and_exit_codes() {
    let (Some(corpus), Some(definition)) = (corpus(), definition()) else {
        return;
    };
    let builder = Builder::new(definition.symbols());
    for (project, template) in templates(&definition) {
        let expected = read(&corpus.join("kasmer").join(project).join("template.kore"));
        let actual = template.with_variable();
        if actual != expected {
            panic!(
                "{project} template: {}",
                first_difference(&actual, &expected)
            );
        }
        for call in calls(&corpus, project) {
            let steps = call_steps(&builder, &call.endpoint, &call.args).unwrap();
            let output = definition
                .interpreter()
                .run_text(&template.fill(&steps), None)
                .unwrap();
            assert_eq!(
                output.exit_code,
                Some(call.exit_code),
                "{project} {} {:?}",
                call.endpoint,
                call.args
            );
        }
    }
}
