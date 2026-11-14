#!/usr/bin/env bash
# Runs tests inside the container. Run scripts/build.sh first.
#
#   scripts/test.sh                  semantics + symbolic
#   scripts/test.sh quick            sanity check, as run by CI: .wast files and scenarios
#                                    (needs only: scripts/build.sh quick)
#   scripts/test.sh semantics        concrete execution of the VM model: .wast files,
#                                    scenarios, Python unit test
#   scripts/test.sh symbolic         symbolic execution: integration, booster, unit
#   scripts/test.sh lemmas           lemma tests and lemma proofs (needs: scripts/build.sh lemmas)
#
# For a single suite, extra arguments are passed on to pytest, e.g.
#   scripts/test.sh symbolic -k call_add

source "$(dirname "${BASH_SOURCE[0]}")/lib.sh"

ensure_container
in_container scripts/inside/test.sh "$@"
