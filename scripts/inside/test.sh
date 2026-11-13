#!/usr/bin/env bash
# Runs inside the container. See scripts/test.sh.

source "$(dirname "${BASH_SOURCE[0]}")/env.sh"
activate_venv

if [ "$HALF" = mx-semantics ]; then
    cd "$ROOT/mx-semantics"

    echo ">> simple tests (.wast, LLVM backend)"
    for f in tests/simple/*.wast; do
        kelrond run --backend llvm "$f" > /dev/null
        echo "   ok  $f"
    done

    echo ">> scenario tests (LLVM backend)"
    mandos tests/mandos/*.scen.json
    echo "   ok  $(ls tests/mandos/*.scen.json | wc -l) scenarios"

    echo ">> Python tests"
    (cd kmultiversx && pytest src/tests --verbose -p no:cacheprovider)
elif [ "${1:-}" = lemmas ]; then
    shift
    cd "$ROOT/kasmer-multiversx/kmxwasm"

    for target in lemma-tests lemma-proofs; do
        kdist which "mxwasm-semantics.$target" > /dev/null 2>&1 || {
            echo "definition missing, run first:" >&2
            echo "  scripts/build.sh kasmer-multiversx mxwasm-semantics.lemma-tests mxwasm-semantics.lemma-proofs" >&2
            exit 1
        }
    done

    echo ">> lemma tests and lemma proofs"
    # The lemma generator (python3 -m kmxwasm.lemmas.generate) is deliberately not run
    # first: its output no longer matches the checked-in proven-mx-lemmas.md.
    pytest src/tests/lemma_tests src/tests/proofs \
        --verbose --durations=10 --numprocesses=4 --dist=worksteal -p no:cacheprovider "$@"
else
    cd "$ROOT/kasmer-multiversx/kmxwasm"

    echo ">> integration, booster and unit tests"
    # test_load_project checks for the example contracts that used to come in
    # through git submodules; deselected until those tests are reworked.
    pytest src/tests/integration src/tests/booster src/tests/unit \
        --deselect src/tests/integration/test_utils.py::test_load_project \
        --verbose --durations=0 --numprocesses=3 --dist=worksteal -p no:cacheprovider "$@"
fi
echo ">> tests done"
