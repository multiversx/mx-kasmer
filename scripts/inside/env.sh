# Common definitions for scripts that run inside the container. Source, do not run.

set -euo pipefail

ROOT=$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)

# One Python environment for everything: kmxwasm depends on kmultiversx by path,
# so installing kmxwasm provides the tools of both.
PROJECT_DIR="$ROOT/kasmer-multiversx/kmxwasm"

# The repository is bind-mounted and owned by a different user id.
git config --global --get-all safe.directory | grep -qxF '*' || git config --global --add safe.directory '*'

# The kompile frontend is a Java program and needs a large heap.
export K_OPTS=${K_OPTS:--Xmx8G -Xss512m}

# Tools are called from the virtual environment directly rather than through `poetry run`.
activate_venv() {
    local venv
    venv=$(cd "$PROJECT_DIR" && poetry env info --path 2>/dev/null) || {
        echo "Python environment missing, run scripts/build.sh first" >&2
        exit 1
    }
    # shellcheck disable=SC1091
    set +u; source "$venv/bin/activate"; set -u
}
