# Wasm test modules

The `.wat` files of the binary parser tests of
[wasm-semantics](https://github.com/runtimeverification/wasm-semantics),
`pykwasm/src/tests/integration/binary/`, at release v0.1.157 (commit `212271bd43`), the release
vendored in this repository. Unmodified.

`scripts/kore-corpus.sh` compiles them with `wat2wasm` and converts them with pykwasm, as reference
data for `tests/conformance.rs`. They cover what the test contracts do not: floats, tables, element
and data segment forms, imports of every kind, globals.
