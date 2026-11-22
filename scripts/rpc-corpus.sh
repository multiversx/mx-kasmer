#!/usr/bin/env bash
# Records proofs of the Python tooling against kore-rpc-booster into .conformance/rpc/: every
# JSON-RPC exchange, how pyk read each execution result, and the final proof graph. The Rust
# crates kore-rpc and kcfg check themselves against it (see scripts/inside/rpc-corpus.py).
#
#   scripts/rpc-corpus.sh [claim...]     default: test_call_add
#
# Needs the targets mxwasm-semantics.haskell, llvm and llvm-library (scripts/build.sh). Takes a
# few minutes per claim.

source "$(dirname "${BASH_SOURCE[0]}")/lib.sh"

ensure_container
in_container bash -c 'source scripts/inside/env.sh; activate_venv
    python scripts/inside/rpc-corpus.py .conformance/rpc "$@"' rpc-corpus "$@"
