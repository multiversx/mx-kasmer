#!/usr/bin/env bash
# Runs inside the container. See scripts/build.sh.

source "$(dirname "${BASH_SOURCE[0]}")/env.sh"

if [ "$HALF" = mx-semantics ]; then
    "$ROOT/scripts/fetch-plugin.sh"
    DEFAULT_TARGETS=(
        mx-semantics.llvm-mandos
        mx-semantics.llvm-kasmer
        mx-semantics.haskell-mandos
        mx-semantics.haskell-kasmer
    )
else
    # mx-semantics.llvm-kasmer comes from the mx-semantics release this half is
    # pinned to (fetched by Poetry), not from the directory next to it.
    DEFAULT_TARGETS=(
        mxwasm-semantics.haskell
        mxwasm-semantics.llvm
        mxwasm-semantics.llvm-library
        mx-semantics.llvm-kasmer
    )
fi

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

TARGETS=("$@")
[ ${#TARGETS[@]} -eq 0 ] && TARGETS=("${DEFAULT_TARGETS[@]}")

echo ">> compiling: ${TARGETS[*]}"
# Two at a time: each kompile can use more than 8 GB.
time kdist -v build "${TARGETS[@]}" -j2
echo ">> build done"
