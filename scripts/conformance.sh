#!/usr/bin/env bash
# Scenario conformance: runs the scenarios of an mx-sdk-rs checkout on the VM model, to check that
# the model behaves like the real VM on the framework's own tests.
#
#   scripts/conformance.sh <mx-sdk-rs-dir> [list-file | scenario.scen.json]...
#
# A list file has one scenario per line, relative to the root of the mx-sdk-rs checkout (a leading
# `deps/mx-sdk-rs/` is ignored, so the lists in mx-semantics/tests/*.test work). Without
# further arguments all four of those lists are run. The contracts the scenarios refer to must
# be built already (`sc-meta all build`).
#
# The checkout is mounted read-only into a throwaway container started from the development
# image, with the build products of scripts/build.sh. Needs mx-semantics.llvm-mandos. Prints one
# line per scenario and a summary; exits with status 1 if any scenario fails or cannot run.
#
# This is the interface meant for mx-sdk-rs CI: it needs only a Docker image with the compiled
# definitions, nothing from this repository's sources but this script.

source "$(dirname "${BASH_SOURCE[0]}")/lib.sh"
[ $# -ge 1 ] || { sed -n '2,/^$/p' "$0" | sed 's/^# \{0,1\}//' >&2; exit 2; }

SDK=$(cd "$1" && pwd); shift
if [ $# -eq 0 ]; then
    set -- "$IN_CONTAINER_ROOT"/mx-semantics/tests/{basic_features,alloc_features,composability_features,multisig}.test
else
    args=()
    for a in "$@"; do
        case "$a" in
            *.test|*.txt) args+=("$IN_CONTAINER_ROOT/$(realpath --relative-to="$ROOT" "$a" 2>/dev/null || echo "$a")") ;;
            *) args+=("$a") ;;
        esac
    done
    set -- "${args[@]}"
fi

ensure_image
docker run --rm --platform linux/amd64 \
    --volume "$ROOT:$IN_CONTAINER_ROOT" \
    --volume "$SDK:/sdk:ro" \
    --volume "$CACHE_MOUNT:/home/user/.cache" \
    --workdir "$IN_CONTAINER_ROOT" \
    "$IMAGE" "$IN_CONTAINER_ROOT/scripts/inside/conformance.sh" "$@"
