#!/usr/bin/env bash
# Fetches the blockchain-k-plugin sources (crypto hooks for the LLVM backend) at
# the pinned commit, including its nested dependencies (cryptopp, libff, cpp-httplib).
#
# Upstream this was a git submodule. It is fetched on demand instead, into a
# git-ignored directory, until the plugin can be dropped (see PLAN.md, step 2.4).

set -euo pipefail

ROOT=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
PIN_FILE="$ROOT/mx-semantics/deps/blockchain-k-plugin_release"
DEST="$ROOT/mx-semantics/kmultiversx/src/kmultiversx/kdist/plugin"
COMMIT=$(cat "$PIN_FILE")

if [ -f "$DEST/.fetched-commit" ] && [ "$(cat "$DEST/.fetched-commit")" = "$COMMIT" ]; then
    echo "plugin already at $COMMIT"
    exit 0
fi

rm -rf "$DEST"
git clone https://github.com/runtimeverification/blockchain-k-plugin.git "$DEST"
git -C "$DEST" checkout --quiet "$COMMIT"
git -C "$DEST" submodule update --init --recursive
echo "$COMMIT" > "$DEST/.fetched-commit"
echo "plugin fetched at $COMMIT"
