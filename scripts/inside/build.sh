#!/usr/bin/env bash
# Runs inside the container. See scripts/build.sh.

source "$(dirname "${BASH_SOURCE[0]}")/env.sh"

DEFAULT_TARGETS=(
    mx-semantics.llvm-mandos       # concrete VM model, scenario runner
    mx-semantics.llvm-kasmer       # concrete VM model with the test hooks: setup, fuzzing
    mxwasm-semantics.haskell       # symbolic definition, with lemmas
    mxwasm-semantics.llvm          # concrete helper for Wasm instantiation during proofs
    mxwasm-semantics.llvm-library  # shared library for the booster backend
)
# What CI builds: the quickest concrete definition, and the symbolic one, which
# checks that the whole VM model and all lemmas still compile.
QUICK_TARGETS=(
    mx-semantics.llvm-mandos
    mxwasm-semantics.haskell
)
LEMMA_TARGETS=(
    mxwasm-semantics.lemma-tests
    mxwasm-semantics.lemma-proofs
)

echo ">> installing Python dependencies ($PROJECT_DIR)"
POETRY_LOG=$(mktemp)
if ! (cd "$PROJECT_DIR" && poetry install --no-ansi --no-interaction) > "$POETRY_LOG" 2>&1; then
    cat "$POETRY_LOG"
    echo "!! poetry install failed" >&2
    exit 1
fi
grep -E "^Installing the current project|^No dependencies" "$POETRY_LOG" || true
rm -f "$POETRY_LOG"
activate_venv

case "${1:-}" in
    "")     TARGETS=("${DEFAULT_TARGETS[@]}") ;;
    quick)  TARGETS=("${QUICK_TARGETS[@]}") ;;
    lemmas) TARGETS=("${LEMMA_TARGETS[@]}") ;;
    *)      TARGETS=("$@") ;;
esac

echo ">> compiling: ${TARGETS[*]}"
# Each kompile can use more than 8 GB: two at a time only with memory to spare.
MEM_GB=$(awk '/^MemTotal:/ { print int($2 / 1024 / 1024) }' /proc/meminfo)
JOBS=1
[ "$MEM_GB" -ge 20 ] && JOBS=2
echo ">> ${MEM_GB} GB of memory, building $JOBS at a time"
time kdist -v build "${TARGETS[@]}" -j"$JOBS"
echo ">> build done"
