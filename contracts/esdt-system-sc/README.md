# ESDT system contract model

A Rust stand-in for the protocol's ESDT system smart contract (issuing tokens, setting roles),
which on the real chain is Go code in
[mx-chain-go](https://github.com/multiversx/mx-chain-go) (`vm/systemSmartContracts/esdt.go`).
It implements only what the scenarios and test contracts need.

Unlike the other contracts here, the source lives in this repository: there is no other copy.
It was written for [runtimeverification/mx-semantics](https://github.com/runtimeverification/mx-semantics)
in PR #242 (2024-04-30, "Implement NFT functionality") under `src/esdt-system-sc/`, and was
moved here from `mx-semantics/src/esdt-system-sc/`.

## Building

```sh
sc-meta all build --wasm-symbols --no-wasm-opt
```

Builds against `multiversx-sc` from crates.io (version in `Cargo.toml`), with `ei = "ignore"` in
`sc-config.toml`. Only `output/esdt-system-sc.wasm` and the ABI are checked in.

## Where it is used

- `scenarios/esdt_system_sc.scen.json`, run by `scripts/test.sh semantics`, deploys
  `output/esdt-system-sc.wasm`.
- The initial configurations in `mx-semantics/kmultiversx/src/kmultiversx/kdist/runtime/*.json`
  have the contract already deployed. They still hold an older build (the path recorded in them,
  `src/esdt-system-sc/...`, is only a name); the generator for them was removed with the old
  Makefiles.
