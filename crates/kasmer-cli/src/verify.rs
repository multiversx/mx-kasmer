//! `kasmer-rs verify`: a symbolic proof per test endpoint.

use std::path::PathBuf;
use std::time::Instant;

use kasmer::claim;
use kasmer::project::Project;
use kasmer::prove::{self, Event, How, Prover, Status};
use kasmer::steps::Builder;
use kasmer::{Error, LlvmDefinition};
use kcfg::RuleLabels;
use kore_rpc::server::{self, Server};

pub struct Options {
    pub server: PathBuf,
    pub proofs: PathBuf,
    pub server_log: Option<PathBuf>,
    pub transcript: Option<PathBuf>,
    pub verbose: bool,
    pub prove: prove::Options,
}

pub fn verify_all(
    definition: &LlvmDefinition,
    project: &Project,
    deployed: &kore::Pattern,
    tests: &[String],
    options: &Options,
) -> Result<bool, Error> {
    for t in tests {
        if !project.endpoints.iter().any(|e| &e.name == t) {
            return Err(Error::Project(format!("no test endpoint {t}")));
        }
    }
    let t = Instant::now();
    let path = options.server.join(server::DEFINITION_FILE);
    let text = std::fs::read_to_string(&path).map_err(|source| Error::Io {
        path: path.clone(),
        source,
    })?;
    let symbolic = kore::text::parse_definition(&text).map_err(|source| Error::Parse {
        what: path.display().to_string(),
        source,
    })?;
    let labels = RuleLabels::from_definition(&symbolic);
    drop(symbolic);
    let server = Server::start(
        &options.server,
        &server::Options {
            args: prove::BOOSTER_ARGS.iter().map(|a| a.to_string()).collect(),
            log: options.server_log.clone(),
            ..Default::default()
        },
    )
    .map_err(rpc_error)?;
    let mut client = server.connect().map_err(rpc_error)?;
    if let Some(path) = &options.transcript {
        let file = std::fs::File::create(path).map_err(|source| Error::Io {
            path: path.clone(),
            source,
        })?;
        client.set_transcript(file);
    }
    eprintln!("started kore-rpc-booster in {:.1?}", t.elapsed());

    let b = Builder::new(definition.symbols());
    let mut failed = 0;
    let mut ran = 0;
    for endpoint in &project.endpoints {
        if !tests.is_empty() && !tests.contains(&endpoint.name) {
            continue;
        }
        ran += 1;
        let claim = claim::build(&b, deployed, endpoint)?;
        let dir = options.proofs.join(&endpoint.name);
        let mut prover = Prover {
            definition,
            client: &mut client,
            labels: &labels,
            options: options.prove.clone(),
        };
        let start = Instant::now();
        let verbose = options.verbose;
        let mut progress = |e: Event| match e {
            Event::Resumed { nodes } => {
                eprintln!(
                    "{}: resuming the saved proof ({nodes} nodes)",
                    endpoint.name
                )
            }
            Event::Processing { node, how, next } if verbose => {
                let how = match how {
                    How::WasmInit => "instantiating Wasm",
                    How::OneStep => "one step",
                    How::Abstracted => "executing",
                };
                eprintln!("  node {node}: {how} ({})", next.unwrap_or_default());
            }
            Event::Processed { node, new, elapsed } if verbose => {
                eprintln!("  node {node}: -> {new:?} in {elapsed:.1?}")
            }
            Event::Covered { node } if verbose => eprintln!("  node {node}: reaches the target"),
            _ => {}
        };
        let outcome = prover.prove(&claim, &dir, &mut progress)?;
        let summary = format!(
            "{} nodes, {} processed in {:.1?}; proof in {}",
            outcome.kcfg.len(),
            outcome.iterations,
            start.elapsed(),
            dir.display()
        );
        match &outcome.status {
            Status::Proved => println!("{}: proved ({summary})", endpoint.name),
            Status::Failed { stuck } => {
                failed += 1;
                println!(
                    "{}: FAILED, node {stuck} is stuck ({summary})",
                    endpoint.name
                );
                if let Ok(node) = outcome.kcfg.node(*stuck) {
                    let c = &node.cterm.config;
                    println!(
                        "  next: {}",
                        prove::next_step(c).unwrap_or("nothing".into())
                    );
                    for cell in ["vmOutput", "logging"] {
                        println!("  {cell}: {}", shorten(&kasmer::cells::cell_text(c, cell)));
                    }
                }
            }
            Status::Incomplete { frontier } => {
                failed += 1;
                println!(
                    "{}: INCOMPLETE, the iteration limit was reached; open: {frontier:?} ({summary})",
                    endpoint.name
                );
            }
        }
    }
    if failed > 0 {
        println!("{failed} of {ran} tests not proved");
    }
    Ok(failed == 0)
}

fn shorten(s: &str) -> String {
    if s.len() <= 1000 {
        return s.to_owned();
    }
    format!(
        "{}... ({} bytes)",
        &s[..s.floor_char_boundary(1000)],
        s.len()
    )
}

fn rpc_error(e: kore_rpc::Error) -> Error {
    Error::Proof(e.to_string())
}
