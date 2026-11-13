#!/usr/bin/env bash
# Installs the Python tooling and compiles the K definitions of one half,
# inside its container. Safe to re-run: only what changed is rebuilt.
#
#   scripts/build.sh mx-semantics
#   scripts/build.sh kasmer-multiversx
#
# Extra arguments are passed on as kdist target names, to build a subset, e.g.
#   scripts/build.sh kasmer-multiversx mxwasm-semantics.haskell

source "$(dirname "${BASH_SOURCE[0]}")/lib.sh"
select_half "${1:-}" || usage_half "[kdist-target...]"
shift

check_docker_memory
ensure_container
in_container scripts/inside/build.sh "$@"
