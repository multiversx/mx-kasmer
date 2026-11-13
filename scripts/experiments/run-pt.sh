#!/usr/bin/env bash
# Tries a parametric test contract, compiled elsewhere, against the tooling:
# fuzzing first, then symbolic verification of each test endpoint.
#
#   scripts/experiments/run-pt.sh <test-contract-dir> <contract.wasm>...
#
# Example, with an mx-sdk-rs checkout next to this repository:
#   scripts/experiments/run-pt.sh \
#       ../mx-sdk-rs/contracts/parametric-tests/adder-pt \
#       ../mx-sdk-rs/contracts/examples/adder/output/adder.wasm
#
# <test-contract-dir>/output must contain the test contract's .wasm and .abi.json.
# The inputs are copied to a scratch directory (.pt-runs/<name>, git-ignored);
# the originals are not modified. Requires scripts/build.sh kasmer-multiversx.

source "$(dirname "${BASH_SOURCE[0]}")/../lib.sh"
[ $# -ge 2 ] || { echo "usage: $(basename "$0") <test-contract-dir> <contract.wasm>..." >&2; exit 2; }
select_half kasmer-multiversx

TEST_DIR=$(cd "$1" && pwd); shift
NAME=$(basename "$TEST_DIR")
RUN="$ROOT/.pt-runs/$NAME"
rm -rf "$RUN"
mkdir -p "$RUN/test/output" "$RUN/contracts"
cp "$TEST_DIR"/output/*.wasm "$TEST_DIR"/output/*.abi.json "$RUN/test/output/"
for w in "$@"; do cp "$w" "$RUN/contracts/"; done
# `kasmer verify` insists on a kasmer.json listing existing contract directories.
echo '{"contracts": ["../contracts"]}' > "$RUN/test/kasmer.json"

ensure_container
in_container scripts/experiments/inside-run-pt.sh ".pt-runs/$NAME"
