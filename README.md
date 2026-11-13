# mx-kasmer

Kasmer is a tool for **parametric testing of MultiversX smart contracts**. A test is written once,
with parameters, and is then either fuzzed with random inputs or proven for all inputs by symbolic
execution.

It is built on a formal model of the MultiversX VM written in the
[K Framework](https://kframework.org/), on top of the K semantics of WebAssembly.

> **Status: imported baseline, work in progress.**
> This repository holds the original code as developed by Runtime Verification, merged from two
> repositories. The original tests build and pass in Docker, and the `adder-pt` example from
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

The two top-level directories are the two original repositories, each with its full history.

| Path | Origin | Contents |
|---|---|---|
| `mx-semantics/` | [runtimeverification/mx-semantics](https://github.com/runtimeverification/mx-semantics) at `216530d` | K model of the VM: accounts, contract calls, ESDT, VM hooks, scenario steps, and the test-only hooks (`kasmer.md`). Python package `kmultiversx`: scenario runner, fuzzing, claim generation. |
| `kasmer-multiversx/` | [runtimeverification/kasmer-multiversx](https://github.com/runtimeverification/kasmer-multiversx) at `a67a732` | Lemmas needed for symbolic execution, the proof driver, and the `kasmer` command line tool. Python package `kmxwasm`. Depends on `mx-semantics`. |

Where to start reading:

- VM model: `mx-semantics/kmultiversx/src/kmultiversx/kdist/mx-semantics/`
- Lemmas: `kasmer-multiversx/kmxwasm/src/kmxwasm/kdist/mxwasm-semantics/`
- Proof loop: `kasmer-multiversx/kmxwasm/src/kmxwasm/property_testing/running.py`
- Command line tool: `kasmer-multiversx/kmxwasm/src/kmxwasm/kasmer/`

The README files inside the two directories are the originals. They describe the old setup and are
partly out of date.

## Building and running the tests

Everything runs in Docker. The only requirements on the host are Docker and a shell.

**Docker resources.** Compiling the semantics needs more than 8 GB of memory; give Docker at least
16 GB (24 GB was used during bring-up). The images and build products take about 20 GB of Docker
disk. The K base image exists for amd64 only, so on Apple Silicon everything runs emulated.

The two halves of the repository still need different K versions, so each has its own image,
container and build cache. All scripts take the half as their first argument.

```sh
# 1. Build: installs the Python tooling and compiles the K definitions.
#    First run: about 20 minutes per half, emulated on Apple Silicon. Re-runs only rebuild what changed.
scripts/build.sh mx-semantics
scripts/build.sh kasmer-multiversx

# 2. Test.
scripts/test.sh mx-semantics        # concrete execution: 6 .wast tests, 6 scenarios, 1 Python test
scripts/test.sh kasmer-multiversx   # symbolic execution: integration, booster and unit tests, about 7 minutes
```

The images and containers are created on first use. The repository is mounted into the
containers, so edits on the host take effect on the next `build.sh` or `test.sh`.

Other entry points:

| Command | Purpose |
|---|---|
| `scripts/build.sh kasmer-multiversx mxwasm-semantics.haskell` | Build selected targets only. After changing a lemma, this one target is enough and takes about 2 minutes |
| `scripts/test.sh kasmer-multiversx -k call_add` | Extra arguments go to `pytest` |
| `scripts/shell.sh <half>` | Shell in the container with `kdist`, `kasmer`, `mandos`, `pytest` on the path |
| `scripts/experiments/run-pt.sh <test-contract-dir> <contract.wasm>...` | Fuzz and verify a parametric test contract compiled elsewhere; see below |

### Trying a parametric test from mx-sdk-rs

With an `mx-sdk-rs` checkout whose contracts are already built (`sc-meta all build`):

```sh
scripts/experiments/run-pt.sh \
    ../mx-sdk-rs/contracts/parametric-tests/adder-pt \
    ../mx-sdk-rs/contracts/examples/adder/output/adder.wasm
```

This copies the `.wasm` and ABI files to `.pt-runs/adder-pt/`, fuzzes every `test_*` endpoint,
generates the claims and verifies each one. Verification logs are written next to the copies.
It is a stopgap that bypasses `kasmer build`, which would try to compile the contracts itself.

### Known limitations

- Tests that compile contracts from the old `mx-sdk-rs` submodule are not run: the scenario
  conformance suites of `mx-semantics`, and `test_load_project` in `kasmer-multiversx`.
- The lemma tests of `kasmer-multiversx` have not been run yet.
- Verification occasionally aborts at start-up with `SMT problem: ... Initial check returned
  Unknown`. This is a solver timeout, seen under emulation; run it again.
- `kasmer-multiversx` does not yet use the `mx-semantics` directory next to it. It installs
  `mx-semantics` 0.1.107 from the original repository. Changes under `mx-semantics/` therefore
  affect only `scripts/test.sh mx-semantics`.
- The crypto plugin sources are downloaded on the first build of `mx-semantics`
  (`scripts/fetch-plugin.sh`).

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
  submodule, are missing. `scripts/fetch-plugin.sh` now downloads them at the commit recorded in
  `mx-semantics/deps/blockchain-k-plugin_release`.
- The original CI workflows are under `*/.github/` and are not active here.

## Pinned versions at import

| Dependency | `mx-semantics` | `kasmer-multiversx` |
|---|---|---|
| K Framework | 7.1.140 | 7.1.85 |
| [wasm-semantics](https://github.com/runtimeverification/wasm-semantics) | 0.1.106 | through `mx-semantics` |
| `mx-semantics` | | 0.1.107, fetched by git tag from the original repository |
| blockchain-k-plugin | `c9264b2` | through `mx-semantics` |

The two halves were out of step with each other when imported: `kasmer-multiversx` expects
`mx-semantics` 0.1.107, while the copy in this repository is 0.1.119.

## Roadmap

1. Reproduce the original build in Docker and get the existing tests to pass.
2. Make `kasmer-multiversx` use the in-repository `mx-semantics`, with a single K version.
3. Vendor the remaining external K sources (wasm-semantics, crypto hooks). No git submodules.
4. Upgrade to a current K Framework release.
5. Remove the dependency on `mx-sdk-rs` and on building contracts: the tool takes compiled
   `.wasm` as input.
6. Replace the Python tooling with a Rust command line tool.
7. Distribute prebuilt artifacts, so that users need neither Java, Haskell nor Python.

## License

`kasmer-multiversx/` is distributed under the BSD 3-Clause License, copyright Runtime
Verification, Inc.; see [kasmer-multiversx/LICENSE.md](kasmer-multiversx/LICENSE.md).

The original `mx-semantics` repository does not contain a license file. Licensing for that part is
to be clarified.
