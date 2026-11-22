#!/usr/bin/env bash
# Writes reference KORE, produced by pyk and the LLVM interpreter, into .conformance/kore/.
# The Rust KORE crate checks itself against it: cargo test -p kore --test conformance
#
# Needs the targets mx-semantics.llvm-kasmer and mxwasm-semantics.haskell (scripts/build.sh).

source "$(dirname "${BASH_SOURCE[0]}")/lib.sh"

ensure_container
in_container bash -c 'source scripts/inside/env.sh; activate_venv
    rm -rf .conformance/kore
    python scripts/inside/kore-corpus.py .conformance/kore'
