# Plan

Working plan for bringing Kasmer back to life under MultiversX. Steps are ordered so that each one
can be verified before the next starts. Update the status column as work proceeds.

Background and component overview: see `README.md`, and the analysis document
`contracts/parametric-tests/KASMER-ANALYSIS.md` in the `mx-sdk-rs` working copy.

## Guiding decisions

- One repository, no git submodules. External K sources are vendored as plain copies with the
  upstream version recorded.
- This repository does not build contracts and does not depend on `mx-sdk-rs`. Its input is
  compiled `.wasm` plus ABI.
- Upgrade K **before** rewriting the Python tooling, so that problems in the semantics and lemmas
  are not confused with problems introduced by the rewrite.
- The primary interface is a command line binary with machine-readable output.

## Phase 0: import (done)

| Step | Status |
|---|---|
| Merge both repositories with history, strip submodules | done, tag `import-baseline` |
| README | done |

## Phase 1: reproduce the original build

Goal: the original tests pass, unchanged semantics, original K versions, inside Docker.

| # | Step | Status |
|---|---|---|
| 1.1 | Single development `Dockerfile` at the repository root: K base image, C++ build dependencies, Poetry. No Rust, no `sc-meta`. K version is a build argument | done; `poetry install` works with Poetry 2.5 and the original lock file |
| 1.2 | Bring back the crypto plugin sources (`blockchain-k-plugin` at `c9264b2`). It pulls in cryptopp, libff and cpp-httplib as nested submodules, too much to vendor, so `scripts/fetch-plugin.sh` fetches it into a git-ignored directory until it can be dropped (step 2.4) | done (script) |
| 1.3 | `mx-semantics` with K 7.1.140: build `llvm-mandos`, run `test-simple` and `mandos-test`. These use only `.wast`/`.wat` fixtures, so they need no contracts | done: build 6 min; 6 `.wast` tests and 7 scenario tests pass. Needs Docker at more than 8 GB (24 GB used). With Poetry 2 the Makefile must be called with `POETRY="poetry -P kmultiversx"`, because `-C` now changes directory |
| 1.4 | `mx-semantics`: build `llvm-kasmer` and the Haskell targets | done: `llvm-kasmer` 18 min, both Haskell targets 2 min; the Python unit test passes |
| 1.5 | `kasmer-multiversx` with K 7.1.85, still fetching `mx-semantics` v0.1.107 by git tag: build `mxwasm-semantics.{llvm,llvm-library,haskell}` | done: three targets in 21 min; `poetry install` still fetches `mx-semantics` v0.1.107 and the plugin from GitHub |
| 1.6 | Run `kmxwasm` integration test `test_call_add` (claim is checked in, no contract build needed), then the booster tests and lemma tests | done for integration, booster and unit tests: 13 pass (`test_call_add` in 2 min). 6 `test_load_project` cases fail as expected, they look for the removed submodule directories. Lemma tests not run yet |
| 1.7 | Record build times and memory needs; note anything that had to change to make it build | in progress, see timings in this table (all emulated amd64 on Apple Silicon) |

Notes:

- The K base image exists for amd64 only. On Apple Silicon it runs under emulation, which is slow
  but sufficient for this phase.
- `kompile` asks for 8 to 12 GB of JVM heap. Docker Desktop must be given enough memory.
- Tests that build contracts from the old `mx-sdk-rs` submodule (scenario conformance for adder,
  multisig, basic-features, and others) are skipped in this phase. See phase 4.

## Phase 2: one coherent codebase

| # | Step |
|---|---|
| 2.1 | Make `kmxwasm` depend on the in-repository `kmultiversx` by path, not by git tag. This moves it from `mx-semantics` 0.1.107 to 0.1.119; fix what breaks |
| 2.2 | One K version for both halves (7.1.140), one lock file |
| 2.3 | Vendor `wasm-semantics` K sources (currently delivered through the `pykwasm` Python package) with the upstream tag recorded and a refresh script |
| 2.4 | Check whether the Haskell and LLVM backends' built-in `KRYPTO` hooks cover `Sha256` and `Keccak256`, the only two used. If so, drop the C++ plugin |
| 2.5 | Delete dead code: the function-summarisation approach in `kmxwasm` (`proofs.py` and its helpers), stale READMEs, old CI files, per-repository Dockerfiles, version-bump scripts |
| 2.6 | Flatten the layout. Proposed: `semantics/` (VM model), `lemmas/`, `vendor/`, `tools/` (Python until replaced), `tests/` |
| 2.7 | GitHub Actions CI on our runners: build image, build definitions, fast tests on every pull request; long proofs on a schedule |

## Phase 3: upgrade K

| # | Step |
|---|---|
| 3.1 | Upgrade `wasm-semantics` and K to current releases in as few jumps as the breakage allows; keep the concrete tests green first |
| 3.2 | Regenerate the runtime configuration (`llvm-kasmer.json`) |
| 3.3 | Get symbolic tests green again; repair lemmas where the backend now simplifies differently |
| 3.4 | Re-run the long proofs (multisig, pair) and compare times against phase 1 |

## Phase 4: invert the dependency on mx-sdk-rs

| # | Step |
|---|---|
| 4.1 | Split `kasmer build` into "compile contracts" (leaves this repository) and "generate claims from `.wasm` + ABI" (stays) |
| 4.2 | Run `adder-pt` from `mx-sdk-rs`, built with the current framework, end to end: fuzz and verify. **First attempt on the imported code (2026-11-13):** fuzzing passes for both tests (31 s) once the DataCount section is stripped; claim generation works; `verify` gets stuck, see 4.4. Scripts used: `scripts/experiments/` |
| 4.3 | Measure the VM hook gap against current contracts and add the missing hooks to the model. Known so far: `managedGetAllTransfersCallValue`, `mBufferToSmallIntUnsigned`, `mBufferFromSmallIntUnsigned`, `managedDeployFromSourceContract`, `managedUpgradeFromSourceContract` |
| 4.4 | Accept the Wasm that current Rust compilers produce. Two concrete problems found with `adder-pt`: (a) the old Wasm parser rejects the DataCount section (id 12); stripping it is safe when `memory.init` and `data.drop` are unused. (b) `i32.extend8_s` (sign-extension operators, on by default since Rust 1.82) runs concretely but has no lemmas: the symbolic backend cannot show `#extends(i32, i8, …)` is defined, takes the `undefined` branch and ends in `trap`. Fix by adding lemmas for `#extends`, or as a stopgap build test contracts without the sign-ext feature |
| 4.5 | ESDT system contract: keep the compiled `.wasm` checked in; build its source against crates.io |
| 4.6 | Scenario conformance suite: run from `mx-sdk-rs` CI against a released image of this repository |
| 4.7 | Framework side: add `setESDTRole` and `checkESDTRole` to the test API; replace `ei = "ignore"` with a proper option; remove legacy `foundry.json` files |

## Phase 5: Rust tooling

| # | Step |
|---|---|
| 5.1 | KORE term model, parser and printer (text and JSON) |
| 5.2 | Wasm binary to K term, using `wasmparser` |
| 5.3 | Concrete path: initial state, `init` run on the LLVM interpreter, fuzzing. Replaces `kasmer build` and `kasmer fuzz` |
| 5.4 | KORE JSON-RPC client; proof graph with persistence and resume |
| 5.5 | Proof loop with cell abstraction, cut points and concrete Wasm instantiation. Replaces `kasmer verify` |
| 5.6 | Reporting: failing path, path condition, counterexample values; JSON output |
| 5.7 | Remove Python from the run-time path; keep lemma maintenance scripts only if still useful |

## Phase 6: distribution

| # | Step |
|---|---|
| 6.1 | Release pipeline producing, per platform: compiled definitions, LLVM interpreter and library, `kore-rpc-booster`, the Rust binary |
| 6.2 | Runtime Docker image as the first supported way to run, including on macOS |
| 6.3 | Native macOS build of the K backends |
| 6.4 | `sc-meta` integration: install or locate the tool, run it on a test contract, show results |

## Later

- More argument types than `BigUint`, `u32`, `u64`.
- A user-facing way to supply loop invariants.
- Gas.
- Licence of the `mx-semantics` part.
