#!/usr/bin/env bash
# Opens a shell in the container with the Python environment active
# (kdist, mandos, kasmer, pytest, ... on the PATH).

source "$(dirname "${BASH_SOURCE[0]}")/lib.sh"
ensure_container
in_container bash -c 'source scripts/inside/env.sh && activate_venv; exec bash'
