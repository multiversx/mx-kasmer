#!/usr/bin/env bash
# Opens a shell in the container of one half, with its Python environment active
# (kdist, mandos, kasmer, pytest, ... on the PATH).
#
#   scripts/shell.sh mx-semantics
#   scripts/shell.sh kasmer-multiversx

source "$(dirname "${BASH_SOURCE[0]}")/lib.sh"
select_half "${1:-}" || usage_half
ensure_container
in_container bash -c 'source scripts/inside/env.sh && activate_venv; exec bash'
