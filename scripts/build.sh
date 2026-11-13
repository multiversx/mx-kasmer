#!/usr/bin/env bash
# Installs the Python tooling and compiles the K definitions, inside the
# container. Safe to re-run: only what changed is rebuilt.
#
#   scripts/build.sh                 everything the tests need
#   scripts/build.sh <target>...     selected kdist targets only, for example
#                                    scripts/build.sh mxwasm-semantics.haskell
#                                    (enough after changing a lemma)
#   scripts/build.sh lemmas          the two extra definitions for the lemma tests

source "$(dirname "${BASH_SOURCE[0]}")/lib.sh"

check_docker_memory
ensure_container
in_container scripts/inside/build.sh "$@"
