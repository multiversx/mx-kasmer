# Experiments

Throwaway scripts used to try contracts from `mx-sdk-rs` against the imported Python tooling,
without going through `kasmer build` (which calls `sc-meta`). They run inside the dev container,
in the Poetry environment of `kmultiversx` or `kmxwasm`.

- `strip_datacount.py FILE.wasm...` removes the DataCount section in place, which the old Wasm
  parser rejects.
- `fuzz_pt.py TEST_DIR CONTRACT.wasm...` deploys the test contract found in `TEST_DIR/output` and
  fuzzes every `test_*` endpoint. Needs `mx-semantics.llvm-kasmer`.
- `gen_claims_pt.py TEST_DIR CONTRACT.wasm...` writes `TEST_DIR/generated_claims/*-spec.json`,
  after which `kasmer verify <test> --booster` can be run from `TEST_DIR` (it needs a
  `kasmer.json` there).

The contract paths given on the command line are the same strings the test contract receives as
`init` arguments.
