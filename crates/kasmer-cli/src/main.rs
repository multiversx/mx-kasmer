//! `kasmer-rs`: the command line of the `kasmer` library.

use std::path::PathBuf;
use std::process::ExitCode;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use clap::{Parser, Subcommand};
use kasmer::fuzz::{self, Outcome, Template};
use kasmer::project::Project;
use kasmer::setup::{self, Contract};
use kasmer::{Error, LlvmDefinition};

#[derive(Parser)]
#[command(
    version,
    about = "Parametric tests of MultiversX smart contracts on the K semantics of the VM"
)]
struct Cli {
    /// The compiled LLVM definition (mx-semantics.llvm-kasmer): a directory with definition.kore
    /// and the interpreter. On macOS, scripts/llvm-kasmer-host.sh writes one that runs the
    /// interpreter in the development container.
    #[arg(long, env = "KASMER_LLVM_DEFINITION", global = true)]
    definition: Option<PathBuf>,

    /// The test project: the directory with kasmer.json.
    #[arg(short = 'C', long = "directory", default_value = ".", global = true)]
    directory: PathBuf,

    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Calls each test endpoint with generated arguments; fails on the first failing call.
    Fuzz {
        /// Only these test endpoints.
        tests: Vec<String>,
        /// Calls per endpoint.
        #[arg(long, default_value_t = 100)]
        examples: usize,
        /// Seed of the argument generator; a failure is reproduced by the same seed. Random by
        /// default.
        #[arg(long)]
        seed: Option<u64>,
        /// Interpreter processes run in parallel. Default: the number of CPUs.
        #[arg(long, short)]
        jobs: Option<usize>,
        /// Print the whole <k> cell of a failure, not just its start.
        #[arg(long, short)]
        verbose: bool,
    },
    /// Deploys the test contract and prints the configuration it is called from, in KORE.
    Deploy {
        /// Write the configuration here rather than to stdout.
        #[arg(long, short)]
        output: Option<PathBuf>,
        /// KORE JSON rather than KORE text.
        #[arg(long)]
        json: bool,
    },
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    match run(cli) {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => ExitCode::from(1),
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::from(2)
        }
    }
}

/// `Ok(false)`: the tests ran, some failed.
fn run(cli: Cli) -> Result<bool, Error> {
    let Some(definition_dir) = cli.definition else {
        return Err(Error::Project(
            "no definition: pass --definition or set KASMER_LLVM_DEFINITION (see scripts/llvm-kasmer-host.sh)".into(),
        ));
    };
    let t = Instant::now();
    let definition = LlvmDefinition::load(&definition_dir)?;
    let project = Project::load(&cli.directory)?;
    eprintln!("loaded definition and project in {:.1?}", t.elapsed());

    let t = Instant::now();
    let deployed = deploy(&definition, &project)?;
    eprintln!("deployed {} in {:.1?}", project.dir.display(), t.elapsed());

    match cli.command {
        Command::Deploy { output, json } => {
            let text = if json {
                kore::json::term_to_string(&deployed)
            } else {
                deployed.to_string()
            };
            match output {
                Some(path) => std::fs::write(&path, text).map_err(|source| Error::Io { path, source })?,
                None => println!("{text}"),
            }
            Ok(true)
        }
        Command::Fuzz {
            tests,
            examples,
            seed,
            jobs,
            verbose,
        } => {
            let seed = seed.unwrap_or_else(random_seed);
            let mut options = fuzz::Options {
                examples,
                seed,
                ..Default::default()
            };
            if let Some(jobs) = jobs {
                options.jobs = jobs;
            }
            eprintln!("seed {seed}, {} jobs", options.jobs);
            fuzz_all(&definition, &project, &deployed, &tests, &options, verbose)
        }
    }
}

fn deploy(definition: &LlvmDefinition, project: &Project) -> Result<kore::Pattern, Error> {
    let runtime = setup::runtime_config(definition, &Contract::esdt_system_sc())?;
    let test = Contract::read(&project.test_wasm)?;
    let contracts = project
        .contracts
        .iter()
        .map(Contract::read)
        .collect::<Result<Vec<_>, _>>()?;
    setup::deploy(definition, runtime, &test, &contracts)
}

fn fuzz_all(
    definition: &LlvmDefinition,
    project: &Project,
    deployed: &kore::Pattern,
    tests: &[String],
    options: &fuzz::Options,
    verbose: bool,
) -> Result<bool, Error> {
    for t in tests {
        if !project.endpoints.iter().any(|e| &e.name == t) {
            return Err(Error::Project(format!("no test endpoint {t}")));
        }
    }
    let template = Template::new(deployed)?;
    let mut failed = 0;
    let mut ran = 0;
    for endpoint in &project.endpoints {
        if !tests.is_empty() && !tests.contains(&endpoint.name) {
            continue;
        }
        ran += 1;
        let t = Instant::now();
        match fuzz::fuzz_endpoint(definition, &template, endpoint, options)? {
            Outcome::Passed { examples } => {
                println!("{}: passed, {examples} calls ({:.1?})", endpoint.name, t.elapsed());
            }
            Outcome::Failed(f) => {
                failed += 1;
                println!("{}: FAILED at call {} ({:.1?})", endpoint.name, f.example, t.elapsed());
                println!("  arguments: {}", f.args_decimal().join(", "));
                println!(
                    "  exit code: {}",
                    f.exit_code.map_or("none (killed)".into(), |c| c.to_string())
                );
                if let Some(logging) = f.logging().filter(|l| !l.is_empty()) {
                    println!("  logging: {logging}");
                }
                println!("  vm output: {}", shorten(&f.vm_output(), 2000, verbose));
                println!("  k: {}", shorten(&f.k(), 2000, verbose));
            }
        }
    }
    if failed > 0 {
        println!("{failed} of {ran} tests failed (seed {})", options.seed);
    }
    Ok(failed == 0)
}

fn shorten(s: &str, max: usize, verbose: bool) -> String {
    if verbose || s.len() <= max {
        return s.to_owned();
    }
    let end = s.floor_char_boundary(max);
    format!("{}... ({} bytes, --verbose for all)", &s[..end], s.len())
}

fn random_seed() -> u64 {
    let nanos = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_nanos());
    (nanos as u64) ^ u64::from(std::process::id()).rotate_left(32)
}
