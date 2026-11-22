//! Fuzzing: each test endpoint is called with generated arguments, from the configuration in which
//! the test contract is deployed, on the LLVM interpreter. A call passes if the interpreter exits
//! with 0: the steps set the exit code to 1, call the endpoint, check that it succeeded, and set it
//! back to 0 (as `kmultiversx.kasmer`, which runs Hypothesis through pyk's `KFuzz`).
//!
//! Arguments come from a seeded generator; an example is determined by the seed and its index, so
//! a failure is reproduced by the same seed whatever the number of parallel jobs. The first two
//! examples are all zeros and all maximal values; the rest mix boundary values (powers of two and
//! their neighbours), small numbers and random ones. Like `kasmer fuzz`, there is no shrinking.

use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use kore::{Pattern, Sort};

use crate::cells::{self, set_cell};
use crate::project::Endpoint;
use crate::setup::{ROOT_ACCOUNT, TEST_CONTRACT};
use crate::steps::Builder;
use crate::{Error, LlvmDefinition, mandos};

/// Gas limit of a test call.
pub const CALL_GAS: u64 = 5_000_000_000;

/// The argument types the fuzzer generates values for.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ArgType {
    BigUint,
    U32,
    U64,
}

impl ArgType {
    pub fn from_abi(name: &str) -> Option<ArgType> {
        match name {
            "BigUint" => Some(ArgType::BigUint),
            "u32" => Some(ArgType::U32),
            "u64" => Some(ArgType::U64),
            _ => None,
        }
    }

    /// The width of the largest value generated. `BigUint` is unbounded; 256 bits is far enough.
    fn bits(self) -> u32 {
        match self {
            ArgType::BigUint => 256,
            ArgType::U32 => 32,
            ArgType::U64 => 64,
        }
    }
}

/// The argument types of an endpoint, or an error naming the first unsupported one.
pub fn arg_types(endpoint: &Endpoint) -> Result<Vec<ArgType>, Error> {
    endpoint
        .inputs
        .iter()
        .map(|t| {
            ArgType::from_abi(t)
                .ok_or_else(|| Error::Project(format!("{}: cannot generate arguments of type {t}", endpoint.name)))
        })
        .collect()
}

/// The deployed configuration as text, with a hole in `<k>` for the steps of a call.
#[derive(Clone, Debug)]
pub struct Template {
    before: String,
    after: String,
}

/// The variable printed in the hole. The same as `kasmer fuzz`, so that the template is the same.
const HOLE: &str = "VarK'Unds'STEPS";

impl Template {
    pub fn new(deployed: &Pattern) -> Result<Template, Error> {
        let mut config = deployed.clone();
        set_cell(&mut config, "k", Pattern::evar(HOLE, Sort::app0("SortK")))?;
        let text = config.to_string();
        let hole = format!("{HOLE} : SortK{{}}");
        let mut parts = text.split(&hole);
        match (parts.next(), parts.next(), parts.next()) {
            (Some(before), Some(after), None) => Ok(Template {
                before: before.to_owned(),
                after: after.to_owned(),
            }),
            _ => Err(Error::Project(format!(
                "{hole} occurs other than once in the configuration"
            ))),
        }
    }

    /// With the hole as the variable `K_STEPS`, as `kasmer fuzz` prints it.
    pub fn with_variable(&self) -> String {
        format!("{}{HOLE} : SortK{{}}{}", self.before, self.after)
    }

    pub fn fill(&self, steps: &Pattern) -> String {
        let steps = steps.to_string();
        let mut s = String::with_capacity(self.before.len() + steps.len() + self.after.len());
        s.push_str(&self.before);
        s.push_str(&steps);
        s.push_str(&self.after);
        s
    }
}

/// The steps of one test call: `setExitCode 1`, the call, a check that it succeeded,
/// `setExitCode 0`. Arguments are big-endian numbers, with no leading zeros.
pub fn call_steps<B: AsRef<[u8]>>(builder: &Builder, endpoint: &str, args: &[B]) -> Result<Pattern, Error> {
    let steps = vec![
        builder.set_exit_code(1)?,
        builder.call_tx(
            &mandos::address(ROOT_ACCOUNT),
            &mandos::sc_address(TEST_CONTRACT),
            0,
            endpoint,
            args,
            CALL_GAS,
            0,
        )?,
        builder.check_expect_ok()?,
        builder.set_exit_code(0)?,
    ];
    Ok(builder.kseq(steps).pattern)
}

#[derive(Clone, Debug)]
pub struct Options {
    /// Examples per endpoint.
    pub examples: usize,
    pub seed: u64,
    /// Interpreter processes run in parallel.
    pub jobs: usize,
}

impl Default for Options {
    fn default() -> Options {
        Options {
            examples: 100,
            seed: 0,
            jobs: std::thread::available_parallelism().map_or(1, |n| n.get()),
        }
    }
}

#[derive(Debug)]
pub enum Outcome {
    Passed { examples: usize },
    Failed(Box<Failure>),
}

/// The first failing example.
#[derive(Debug)]
pub struct Failure {
    pub example: usize,
    /// Big-endian, no leading zeros.
    pub args: Vec<Vec<u8>>,
    pub exit_code: Option<i32>,
    /// The final configuration.
    pub config: Pattern,
    pub stderr: String,
}

impl Failure {
    pub fn args_decimal(&self) -> Vec<String> {
        self.args.iter().map(|a| mandos::to_decimal(a)).collect()
    }

    pub fn k(&self) -> String {
        cells::cell_text(&self.config, "k")
    }

    pub fn vm_output(&self) -> String {
        cells::cell_text(&self.config, "vmOutput")
    }

    pub fn logging(&self) -> Option<&str> {
        cells::string_cell(&self.config, "logging")
    }
}

/// Fuzzes one endpoint. Stops at the first failure.
pub fn fuzz_endpoint(
    definition: &LlvmDefinition,
    template: &Template,
    endpoint: &Endpoint,
    options: &Options,
) -> Result<Outcome, Error> {
    let types = arg_types(endpoint)?;
    let builder = Builder::new(definition.symbols());
    let examples = if types.is_empty() { 1 } else { options.examples };
    let next = AtomicUsize::new(0);
    let stop = AtomicBool::new(false);
    // The failure with the lowest index wins. Examples are started in order, so every example
    // before a failing one has been started and will be finished: the result does not depend on
    // the number of jobs.
    let failure: Mutex<Option<Failure>> = Mutex::new(None);
    let error: Mutex<Option<Error>> = Mutex::new(None);

    let run_example = |index: usize| -> Result<Option<Failure>, Error> {
        let args = example_args(options.seed, index, &types);
        let steps = call_steps(&builder, &endpoint.name, &args)?;
        let output = definition.interpreter().run_text(&template.fill(&steps), None)?;
        if output.exit_code == Some(0) {
            return Ok(None);
        }
        // Without a final configuration the interpreter did not run the test: it failed itself
        // (or could not be started, for a wrapper).
        let config = output.config()?;
        Ok(Some(Failure {
            example: index,
            args,
            exit_code: output.exit_code,
            config,
            stderr: output.stderr,
        }))
    };

    std::thread::scope(|s| {
        for _ in 0..options.jobs.clamp(1, examples) {
            s.spawn(|| {
                while !stop.load(Ordering::Relaxed) {
                    let index = next.fetch_add(1, Ordering::Relaxed);
                    if index >= examples {
                        break;
                    }
                    match run_example(index) {
                        Ok(None) => {}
                        Ok(Some(f)) => {
                            stop.store(true, Ordering::Relaxed);
                            let mut slot = failure.lock().unwrap();
                            if slot.as_ref().is_none_or(|old| f.example < old.example) {
                                *slot = Some(f);
                            }
                        }
                        Err(e) => {
                            stop.store(true, Ordering::Relaxed);
                            error.lock().unwrap().get_or_insert(e);
                        }
                    }
                }
            });
        }
    });

    if let Some(e) = error.into_inner().unwrap() {
        return Err(e);
    }
    Ok(match failure.into_inner().unwrap() {
        Some(f) => Outcome::Failed(Box::new(f)),
        None => Outcome::Passed { examples },
    })
}

/// The arguments of example `index`.
pub fn example_args(seed: u64, index: usize, types: &[ArgType]) -> Vec<Vec<u8>> {
    let mut rng = Rng::new(seed ^ (index as u64).wrapping_mul(0x9e37_79b9_7f4a_7c15));
    types
        .iter()
        .map(|&t| match index {
            0 => Vec::new(),
            1 => ones(t.bits()),
            _ => generate(&mut rng, t),
        })
        .collect()
}

fn generate(rng: &mut Rng, t: ArgType) -> Vec<u8> {
    let bits = t.bits();
    match rng.below(8) {
        // A boundary: 2^k - 1 or 2^k, for k up to the width.
        0 | 1 => {
            let k = rng.below(u64::from(bits) + 1) as u32;
            if k == bits || rng.below(2) == 0 {
                ones(k)
            } else {
                power_of_two(k)
            }
        }
        // Small.
        2 | 3 => mandos::uint_bytes(&rng.below(257).to_be_bytes()).to_vec(),
        // Random, of a random width.
        _ => {
            let k = 1 + rng.below(u64::from(bits)) as u32;
            let mut v: Vec<u8> = (0..k.div_ceil(8)).map(|_| rng.next() as u8).collect();
            let top_bits = k - 8 * (k.div_ceil(8) - 1);
            v[0] &= ((1u16 << top_bits) - 1) as u8;
            mandos::uint_bytes(&v).to_vec()
        }
    }
}

/// 2^k - 1, big-endian.
fn ones(k: u32) -> Vec<u8> {
    let mut v = vec![0xffu8; k.div_ceil(8) as usize];
    if !k.is_multiple_of(8) {
        v[0] = (1u8 << (k % 8)) - 1;
    }
    v
}

/// 2^k, big-endian.
fn power_of_two(k: u32) -> Vec<u8> {
    let mut v = vec![0u8; k as usize / 8 + 1];
    v[0] = 1 << (k % 8);
    v
}

/// SplitMix64.
struct Rng(u64);

impl Rng {
    fn new(seed: u64) -> Rng {
        Rng(seed)
    }

    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }

    /// Uniform in `0..n`.
    fn below(&mut self, n: u64) -> u64 {
        ((u128::from(self.next()) * u128::from(n)) >> 64) as u64
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn values_fit() {
        for t in [ArgType::BigUint, ArgType::U32, ArgType::U64] {
            for i in 0..2000 {
                let [v] = &example_args(7, i, &[t])[..] else { panic!() };
                assert!(v.first() != Some(&0), "{t:?} {v:?}: leading zero");
                assert!(v.len() * 8 <= t.bits() as usize, "{t:?} {v:?}: too wide");
            }
        }
        assert_eq!(example_args(1, 1, &[ArgType::U32]), vec![vec![0xff; 4]]);
        assert_eq!(
            example_args(1, 0, &[ArgType::U32, ArgType::BigUint]),
            vec![Vec::<u8>::new(), Vec::new()]
        );
    }

    #[test]
    fn boundaries() {
        assert_eq!(ones(0), Vec::<u8>::new());
        assert_eq!(ones(9), vec![1, 0xff]);
        assert_eq!(power_of_two(0), vec![1]);
        assert_eq!(power_of_two(8), vec![1, 0]);
        assert_eq!(power_of_two(10), vec![4, 0]);
    }

    #[test]
    fn deterministic() {
        let t = [ArgType::BigUint, ArgType::U64];
        assert_eq!(example_args(42, 17, &t), example_args(42, 17, &t));
        assert_ne!(example_args(42, 17, &t), example_args(43, 17, &t));
    }
}
