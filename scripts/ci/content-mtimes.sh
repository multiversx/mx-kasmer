#!/usr/bin/env bash
# Gives every tracked file a modification time derived from its content.
#
# The build tool (kdist) decides whether a compiled definition is up to date by
# comparing the modification times of its source files with those recorded at
# the last build. A fresh checkout stamps every file with the current time, so
# a restored build cache would never be considered up to date. After this
# script, a file has the same timestamp in every checkout as long as its
# content is the same, and a different one when it changes.
#
# Meant for CI, on a fresh checkout. Harmless but pointless on a working copy.

set -euo pipefail

cd "$(dirname "${BASH_SOURCE[0]}")/../.."

# Timestamps land between 2001 and 2010: 10^9 seconds plus 28 bits of the blob hash.
git ls-files -s -z | while IFS= read -r -d '' entry; do
    meta=${entry%%$'\t'*}
    path=${entry#*$'\t'}
    read -r mode hash _ <<< "$meta"
    [ "$mode" = 120000 ] && continue    # symbolic link
    [ -f "$path" ] || continue
    touch -d "@$((1000000000 + 16#${hash:0:7}))" "$path"
done
