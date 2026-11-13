#!/usr/bin/env bash
# Runs the tests of one half inside its container. Run scripts/build.sh first.
#
#   scripts/test.sh mx-semantics        concrete tests: .wast files, scenarios, Python unit test
#   scripts/test.sh kasmer-multiversx   symbolic tests: integration, booster, unit
#
# Extra arguments are passed on to pytest (kasmer-multiversx only), e.g.
#   scripts/test.sh kasmer-multiversx -k call_add

source "$(dirname "${BASH_SOURCE[0]}")/lib.sh"
select_half "${1:-}" || usage_half "[pytest-args...]"
shift

ensure_container
in_container scripts/inside/test.sh "$@"
