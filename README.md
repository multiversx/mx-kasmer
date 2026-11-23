# mx-kasmer

Kasmer is a tool for **parametric testing of MultiversX smart contracts**. A test is written once,
with parameters, and is then either fuzzed with random inputs or proven for all inputs by symbolic
execution.

It is built on a formal model of the MultiversX VM written in the
[K Framework](https://kframework.org/), on top of the K semantics of WebAssembly.

> **Status: work in progress.**
> This repository holds the code originally developed by Runtime Verification, merged from two
> repositories and since unified: one K version, one build, no git submodules and no downloads of
> source code at build time. The tests build and pass in Docker, and the `adder-pt` example from
> `mx-sdk-rs` can be fuzzed and verified. The tooling is still the original Python. See
> [Building and running the tests](#building-and-running-the-tests), [Roadmap](#roadmap) and
> [PLAN.md](PLAN.md).

## How it works

Tests are smart contracts. A test contract deploys the contract under test in its `init` function,
and every endpoint named `test_*` is a parametric test: its arguments are the parameters. Test
contracts have access to extra VM hooks that exist only in the model, such as `assume`, `assert`,
`startPrank`, `createAccount` and `setStorage`. Support for writing them lives in
[mx-sdk-rs](https://github.com/multiversx/mx-sdk-rs) (`self.test_raw()`, `.test_deploy()`,
`.test_call()`; examples under `contracts/parametric-tests`).

Kasmer works on compiled WebAssembly, not on Rust source. Given the `.wasm` files and the test
contract's ABI, it:

1. runs the test contract's `init` concretely to obtain the starting state;
2. for **fuzzing**, calls each test endpoint with generated arguments on the concrete (LLVM)
   backend of K;
3. for **verification**, turns each test endpoint into a reachability claim with symbolic
   arguments and explores every execution path on the symbolic (Haskell) backend of K, using Z3.
   A failed `assert` on any path shows up as a path that cannot reach the success state.

## Repository layout

The two main directories are the two original repositories, each with its full history. They are
built and tested together.

| Path | Contents |
|---|---|
| `mx-semantics/` | The K model of the VM and the Python package `kmultiversx` (scenario runner, fuzzing, claim generation). Originally [runtimeverification/mx-semantics](https://github.com/runtimeverification/mx-semantics) |
| `kasmer-multiversx/` | Lemmas for symbolic execution, the proof driver and the `kasmer` command line tool, in the Python package `kmxwasm`. Depends on `kmultiversx` by path. Originally [runtimeverification/kasmer-multiversx](https://github.com/runtimeverification/kasmer-multiversx) |
| `deps/` | Version pins: `k_release` (K Framework), `kwasm_release` (vendored Wasm semantics) |
| `contracts/` | Compiled test contracts and their scenarios, mirroring the layout of `mx-sdk-rs/contracts/` |
| `scripts/` | Build and test entry points |
| `Dockerfile` | The development image: K plus Python. No Rust toolchain; contracts are compiled elsewhere |

Where to start reading (`K` stands for `mx-semantics/kmultiversx/src/kmultiversx/kdist`):

| What | Where |
|---|---|
| VM model: accounts, calls, ESDT, VM hooks, scenario steps, test-only hooks (`kasmer.md`) | `K/mx-semantics/` |
| K semantics of WebAssembly, vendored unmodified from upstream; see `VENDORED.md` there | `K/wasm-semantics/` |
| Hash functions used by the VM hooks: K declarations and the C++ for the concrete backend | `K/krypto/` |
| Initial configuration with the ESDT system contract deployed (generated, checked in) | `K/runtime/` |
| Lemmas | `kasmer-multiversx/kmxwasm/src/kmxwasm/kdist/mxwasm-semantics/` |
| Proof loop | `kasmer-multiversx/kmxwasm/src/kmxwasm/property_testing/running.py` |
| Command line tool | `kasmer-multiversx/kmxwasm/src/kmxwasm/kasmer/` |
| ESDT system contract model (a Rust contract, with its source) | `contracts/esdt-system-sc/` |
| Test contracts: compiled `.wasm` and ABI, scenarios, `kasmer.json`. Sources are in mx-sdk-rs; see [contracts/README.md](contracts/README.md) | `contracts/` |

## Building and running the tests

Everything runs in Docker. The only requirements on the host are Docker and a shell.

**Docker resources.** Compiling the semantics needs more than 8 GB of memory; give Docker at least
16 GB (24 GB was used during bring-up), and allow about 15 GB of Docker disk. The K base image
exists for amd64 only, so on Apple Silicon everything runs emulated.

```sh
# 1. Build: installs the Python tooling and compiles the K definitions.
#    First run: about 80 minutes emulated on Apple Silicon with 24 GB (definitions are compiled
#    one at a time below 30 GB, two at a time above). Re-runs only rebuild what changed.
scripts/build.sh

# 2. Test (about 7 minutes).
scripts/test.sh

# 3. Optional: lemma tests, lemma proofs and the check that the generated lemmas are up to date
#    (about 12 minutes, plus two more definitions to compile).
scripts/build.sh lemmas
scripts/test.sh lemmas
```

The image, the container (`mx-kasmer-dev`) and the build cache (Docker volume `mx-kasmer-cache`)
are created on first use. The repository is mounted into the container, so edits on the host take
effect on the next `build.sh` or `test.sh`.

| Command | Purpose |
|---|---|
| `scripts/build.sh quick`, `scripts/test.sh quick` | The sanity check that CI runs: two definitions (about 8 minutes emulated), `.wast` tests and scenarios |
| `scripts/test.sh semantics` | Concrete execution of the VM model: `.wast` tests, scenarios (including the crypto hooks), a Python unit test |
| `scripts/test.sh symbolic` | Symbolic execution: integration, booster and unit tests |
| `scripts/test.sh symbolic -k call_add` | For a single suite, extra arguments go to `pytest` |
| `scripts/build.sh mxwasm-semantics.haskell` | Build selected targets only. After changing a lemma, this one target is enough and takes about 2 minutes |
| `scripts/shell.sh` | Shell in the container with `kdist`, `kasmer`, `mandos`, `pytest` on the path |
| `scripts/experiments/run-pt.sh <test-contract-dir> [<contract.wasm>...]` | Fuzz and verify a parametric test contract; see below |
| `scripts/conformance.sh <mx-sdk-rs-dir> [list...]` | Run the scenarios of an `mx-sdk-rs` checkout on the VM model; one line per failure, with what it is stuck on |
| `scripts/update-wasm-semantics.sh <version>` | Replace the vendored Wasm semantics with another upstream release |
| `scripts/migrations/` | One-off conversions of checked-in K terms (JSON) for an upgrade of the semantics. Kept as a record and as a template for the next upgrade |

### Running a parametric test

The test contracts in `contracts/parametric-tests/` come compiled:

```sh
scripts/experiments/run-pt.sh \
    contracts/parametric-tests/adder-pt \
    contracts/examples/adder/output/adder.wasm
```

This copies the `.wasm` and ABI files to `.pt-runs/adder-pt/`, fuzzes every `test_*` endpoint,
generates the claims and verifies each one. Verification logs are written next to the copies.
It is a stopgap around `kasmer claims`, which does the same from a `kasmer.json` project directory
without compiling anything. A test contract from elsewhere, for instance an `mx-sdk-rs` checkout,
must be built first with `sc-meta all build --wasm-symbols --no-wasm-opt`.

The companion `adder-pt-fail` contains tests that are expected to fail and is the check that
failures are actually reported: one test fails under fuzzing and verification, another fails for
a single input that only verification finds. The script exits with status 1 for it.

### Continuous integration

`.github/workflows/test.yml` runs a short sanity check on every push to a pull request, with the
same scripts: `scripts/build.sh quick`, then `scripts/test.sh quick`. It checks that the symbolic
definition compiles, which type-checks the whole VM model and all lemmas, and runs the `.wast` and
scenario tests on the smaller concrete definition.

The symbolic, lemma and parametric test suites are **not** run by CI; run them locally before
merging changes to the semantics, the lemmas or the proof driver. They need the largest definition
(`llvm-kasmer`), which does not compile within the 16 GB of a standard GitHub runner.

Compiled definitions and the Python environment are cached between runs. The build tool compares
file modification times, which a fresh checkout resets, so the workflow first runs
`scripts/ci/content-mtimes.sh` to give every file a timestamp derived from its content. Only
definitions whose sources changed are then rebuilt.

### Rust tooling

The Rust replacement of the Python tooling is being written in `crates/`, a Cargo workspace at the
repository root. It runs on the host; running contracts also needs the compiled LLVM definition,
and symbolic execution the `kore-rpc-booster` server, which for now are in the container:

```sh
cargo test

# Fuzzing: make the definition usable from the host, then fuzz a test project.
scripts/llvm-kasmer-host.sh                       # writes .build/llvm-kasmer/
export KASMER_LLVM_DEFINITION=$PWD/.build/llvm-kasmer
cargo run --release -p kasmer-cli -- -C contracts/parametric-tests/adder-pt fuzz

# Verification: a server directory for the booster in the container, then prove each test
# endpoint for all arguments. Proofs are saved in <project>/.kasmer/proofs and resumed when run
# again (--fresh to start over).
scripts/kore-rpc-host.sh                          # writes .build/kore-rpc/
export KASMER_RPC_SERVER=$PWD/.build/kore-rpc
cargo run --release -p kasmer-cli -- -C contracts/parametric-tests/adder-pt verify -v
```

| Crate | Contents |
|---|---|
| `kore` | KORE term model; text and JSON parsers and printers; symbol signatures and well-sorted term building from K labels |
| `kwasm` | Wasm binary to the K term of its module (replaces pykwasm's `wasm2kast` and pyk's `kast_to_kore`) |
| `kasmer` | The library: test projects, setting up and deploying into the configuration, running the LLVM interpreter, fuzzing; claims and their proofs on the booster (cut points, cell abstraction, Wasm instantiation on the LLVM interpreter). Depends on `kore`, `kwasm`, `kore-rpc`, `kcfg` and `serde_json` |
| `kasmer-cli` | The command line, `kasmer-rs` for now: `fuzz` (replaces `kasmer fuzz`), `verify` (replaces `kasmer claims` and `kasmer verify`) and `deploy` (prints the deployed configuration) |
| `kore-rpc` | Client of the K JSON-RPC servers (`execute`, `implies`, `simplify`, `get-model`, `add-module`), and a launcher that starts a server from a server directory and connects to it, also in a container |
| `kcfg` | Proof graphs in KORE, as pyk's `KCFG`: constrained terms, edges, covers, splits, branches; extending a node from an `execute` result as pyk does; saved to disk as the proof goes, so that a proof can be resumed |

`scripts/llvm-kasmer-host.sh` copies `definition.kore` out of the container and writes an
`interpreter` wrapper that runs the real one there with `docker exec`. Where the K backends run
natively, the compiled definition directory can be used directly.

`scripts/kore-rpc-host.sh` does the same for the booster: a `server` script that starts it in the
container (and stops it when its standard input closes, so no server outlives the host process),
and a `connect` script that forwards a connection to it through `docker exec`, since the container
publishes no ports.

The Python tooling stays until the Rust tool can replace it, and serves as the reference: the Rust
crates are checked against output of pyk. `scripts/kore-corpus.sh` writes that reference data into
`.conformance/kore/` (two compiled definitions, their 34,000 axioms, configurations with deployed
contracts, raw output of the LLVM interpreter, the module terms of every contract in `contracts/`
and of upstream's Wasm test modules, and the fuzzing setup of four test projects with calls and
their exit codes); `cargo test` then checks the crates against it, byte for byte. Without the data
those tests pass with a notice. The tests that run the interpreter also need
`KASMER_LLVM_DEFINITION`.

`scripts/rpc-corpus.sh` records proofs of the Python tooling (`kasmer verify` of two endpoints of
`adder-pt` and the failing one of `adder-pt-fail`, about ten minutes) into `.conformance/rpc/`: the
claim's first two nodes in KORE, every JSON-RPC exchange with the booster, how pyk read each
execution result, and the final proof graph. `kore-rpc` and `kcfg` check themselves against it;
`kasmer` builds the same claims and, with `KASMER_RPC_SERVER` set, proves them with the same
result and the same graph. With `KASMER_RPC_SERVER` set, the recorded requests are also replayed
against a fresh server.

### Known limitations

- Tests that need contracts compiled from the old `mx-sdk-rs` submodule are not run: the scenario
  conformance suites listed in `mx-semantics/tests/*.test`, and `test_load_project`.
- Two lemma proof files are excluded as known broken (`mod-int-total.k`, `helper-lemmas.k`).
- Test runs leave ignored files behind: `*.debug-log` next to the lemma tests (overwritten on each
  run) and `.pt-runs/` (one directory per contract, recreated on each run of that contract, over
  100 MB each). Both can be deleted at any time.
- Verification occasionally aborts at start-up with `SMT problem: ... Initial check returned
  Unknown`. This is a solver timeout, seen under emulation; run it again.
- The Python dependencies, including `pyk` and `pykwasm`, are still downloaded when the
  environment is first installed.

## About the import

- The tag `import-baseline` marks the state right after the merge, before any change of ours.
- Commit hashes differ from the original repositories, because every commit was rewritten to move
  its files into a subdirectory.
- Pull request references in commit messages were rewritten to point at the original repository,
  for example `runtimeverification/mx-semantics#331`.
- Git submodules were removed from the entire history. Commits that only moved a submodule pointer
  were dropped as empty (40 in `mx-semantics`, 2 in `kasmer-multiversx`).
- As a consequence, the tag `import-baseline` does not build as is: the sources of
  [blockchain-k-plugin](https://github.com/runtimeverification/blockchain-k-plugin), previously a
  submodule, are missing.
- The original CI workflows, Dockerfiles, Nix flakes and Makefiles have since been removed. They
  remain in the history.

## Versions

| Dependency | Now | At import |
|---|---|---|
| K Framework | 7.1.337 (`deps/k_release`) | 7.1.140 for `mx-semantics`, 7.1.85 for `kasmer-multiversx` |
| [wasm-semantics](https://github.com/runtimeverification/wasm-semantics) | 0.1.157, vendored (`deps/kwasm_release`) | 0.1.106, through the `pykwasm` Python package |
| `mx-semantics`, as used by `kasmer-multiversx` | the copy in this repository (0.1.119) | 0.1.107, fetched by git tag from the original repository |
| blockchain-k-plugin (crypto hooks, C++) | not used; replaced by `krypto/` in this repository | commit `c9264b2`, a git submodule |

## Roadmap

Done:

1. Reproduce the original build in Docker and get the existing tests to pass.
2. Make `kasmer-multiversx` use the in-repository `mx-semantics`, with a single K version.
3. Bring the remaining external K sources into the repository (Wasm semantics, crypto hooks).
   No git submodules.
4. Upgrade to current releases of the K Framework (7.1.337) and the Wasm semantics (0.1.157).

To do:

5. Remove the dependency on building contracts: the tool takes compiled `.wasm` as input.
6. Replace the Python tooling with a Rust command line tool.
7. Distribute prebuilt artifacts, so that users need neither Java, Haskell nor Python.

## License

`kasmer-multiversx/` is distributed under the BSD 3-Clause License, copyright Runtime
Verification, Inc.; see [kasmer-multiversx/LICENSE.md](kasmer-multiversx/LICENSE.md).

The original `mx-semantics` repository does not contain a license file. Licensing for that part is
to be clarified.

The vendored Wasm semantics are under the University of Illinois/NCSA Open Source License; see
`LICENSE` in their directory.
