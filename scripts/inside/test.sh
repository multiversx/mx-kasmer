#!/usr/bin/env bash
# Runs inside the container. See scripts/test.sh.

source "$(dirname "${BASH_SOURCE[0]}")/env.sh"
activate_venv

require_targets() {
    local hint=$1; shift
    for target in "$@"; do
        kdist which "$target" > /dev/null 2>&1 || {
            echo "definition $target missing, run first: $hint" >&2
            exit 1
        }
    done
}

test_semantics() {
    require_targets "scripts/build.sh" mx-semantics.llvm-mandos
    cd "$ROOT/mx-semantics"

    echo ">> semantics: simple tests (.wast, LLVM backend)"
    for f in tests/simple/*.wast; do
        kelrond run --backend llvm "$f" > /dev/null
        echo "   ok  $f"
    done

    echo ">> semantics: scenario tests (LLVM backend)"
    mandos tests/mandos/*.scen.json
    echo "   ok  $(ls tests/mandos/*.scen.json | wc -l) scenarios"

    echo ">> semantics: Python tests"
    (cd kmultiversx && pytest src/tests --verbose -p no:cacheprovider "$@")
}

test_symbolic() {
    require_targets "scripts/build.sh" mxwasm-semantics.haskell mxwasm-semantics.llvm mxwasm-semantics.llvm-library
    cd "$ROOT/kasmer-multiversx/kmxwasm"

    echo ">> symbolic: integration, booster and unit tests"
    # test_load_project checks for the example contracts that used to come in
    # through git submodules; deselected until those tests are reworked.
    pytest src/tests/integration src/tests/booster src/tests/unit \
        --deselect src/tests/integration/test_utils.py::test_load_project \
        --verbose --durations=0 --numprocesses=3 --dist=worksteal -p no:cacheprovider "$@"
}

test_lemmas() {
    require_targets "scripts/build.sh lemmas" mxwasm-semantics.lemma-tests mxwasm-semantics.lemma-proofs
    cd "$ROOT/kasmer-multiversx/kmxwasm"

    echo ">> lemmas: generated lemmas are up to date"
    # The generator rewrites these two files in place, emptying them first. Work on a
    # snapshot so that a failing or diverging run leaves the working tree as it was.
    local lemmas=src/kmxwasm/kdist/mxwasm-semantics/lemmas
    local generated=("$lemmas/proven-mx-lemmas.md" "$lemmas/proofs/helper-lemmas.k")
    local snapshot; snapshot=$(mktemp -d)
    cp "${generated[@]}" "$snapshot/"
    local outcome=ok
    python3 -m kmxwasm.lemmas.generate > "$snapshot/generate.log" 2>&1 || outcome="the generator failed"
    for f in "${generated[@]}"; do
        if [ "$outcome" = ok ] && ! cmp -s "$f" "$snapshot/$(basename "$f")"; then
            outcome="$f differs from what the generator produces"
            diff "$snapshot/$(basename "$f")" "$f" | head -20 || true
        fi
    done
    for f in "${generated[@]}"; do cp "$snapshot/$(basename "$f")" "$f"; done
    if [ "$outcome" != ok ]; then
        tail -5 "$snapshot/generate.log"
        echo "!! $outcome" >&2
        exit 1
    fi
    rm -rf "$snapshot"

    echo ">> lemmas: lemma tests and lemma proofs"
    pytest src/tests/lemma_tests src/tests/proofs \
        --verbose --durations=10 --numprocesses=4 --dist=worksteal -p no:cacheprovider "$@"
}

case "${1:-}" in
    "")        test_semantics; test_symbolic ;;
    semantics) shift; test_semantics "$@" ;;
    symbolic)  shift; test_symbolic "$@" ;;
    lemmas)    shift; test_lemmas "$@" ;;
    *) echo "unknown suite: $1 (expected semantics, symbolic or lemmas)" >&2; exit 2 ;;
esac
echo ">> tests done"
