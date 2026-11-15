#!/usr/bin/env bash
# Replaces the vendored K semantics of WebAssembly with the given upstream release.
#
#   scripts/update-wasm-semantics.sh 0.1.106
#
# Also update the pykwasm tag in mx-semantics/kmultiversx/pyproject.toml to the
# same release, re-lock, rebuild and run the tests before committing.

set -euo pipefail

VERSION=${1:?usage: update-wasm-semantics.sh <version, e.g. 0.1.106>}
ROOT=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
DEST="$ROOT/mx-semantics/kmultiversx/src/kmultiversx/kdist/wasm-semantics"
TMP=$(mktemp -d)
trap 'rm -rf "$TMP"' EXIT

curl -fsSL "https://github.com/runtimeverification/wasm-semantics/archive/refs/tags/v$VERSION.tar.gz" \
    | tar -xz -C "$TMP" --strip-components=1

cp "$DEST/VENDORED.md" "$TMP/VENDORED.md"
rm -rf "$DEST"
cp -r "$TMP/pykwasm/src/pykwasm/kdist/wasm-semantics" "$DEST"
cp "$TMP/LICENSE" "$TMP/VENDORED.md" "$DEST/"
echo "$VERSION" > "$ROOT/deps/kwasm_release"
echo "vendored wasm-semantics v$VERSION into ${DEST#"$ROOT"/}"
