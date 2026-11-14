# Common definitions for the host-side scripts. Source this file, do not run it.
#
# Everything runs in one long-lived Docker container, created on first use:
#
#   image      mx-kasmer-dev:k<version>   K version taken from deps/k_release
#   container  mx-kasmer-dev
#   volume     mx-kasmer-cache            mounted on ~/.cache in the container
#                                         (or the host directory in KASMER_CACHE)
#
# The repository is bind-mounted into the container, so edits on the host are
# visible immediately. Build products (compiled K definitions, the Python
# virtual environment) live in the volume and survive the container.

set -euo pipefail

ROOT=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
IN_CONTAINER_ROOT=/home/user/mx-kasmer
# A machine sold as 16 GB reports a little less, hence 14.
MIN_DOCKER_MEM_GB=14

K_VERSION=$(cat "$ROOT/deps/k_release")
IMAGE="mx-kasmer-dev:k$K_VERSION"
CONTAINER="mx-kasmer-dev"

# Where build products are kept: a named Docker volume by default. Set
# KASMER_CACHE to a host directory to use that instead (CI does, so that it can
# save and restore the directory between runs).
CACHE_MOUNT="${KASMER_CACHE:-mx-kasmer-cache}"

# On Linux a bind mount keeps the host's user ids, so the user in the image must
# have the id of the host user to be able to write to the repository. Docker
# Desktop on macOS maps ownership itself, and the default id is fine there.
IMAGE_USER_ID=1000
if [ "$(uname -s)" = Linux ]; then
    IMAGE_USER_ID=$(id -u)
fi

ensure_image() {
    if ! docker image inspect "$IMAGE" >/dev/null 2>&1; then
        echo ">> building image $IMAGE"
        docker build --platform linux/amd64 \
            --build-arg K_VERSION="$K_VERSION" --build-arg USER_ID="$IMAGE_USER_ID" \
            -t "$IMAGE" "$ROOT"
    fi
}

check_docker_memory() {
    local bytes gb
    bytes=$(docker info --format '{{.MemTotal}}')
    gb=$((bytes / 1024 / 1024 / 1024))
    if [ "$gb" -lt "$MIN_DOCKER_MEM_GB" ]; then
        echo "!! Docker has ${gb} GB of memory. Compiling the semantics needs more than 8 GB;" >&2
        echo "!! give Docker at least 16 GB (Docker Desktop: Settings > Resources)." >&2
        exit 1
    fi
}

ensure_container() {
    ensure_image
    if docker container inspect "$CONTAINER" >/dev/null 2>&1; then
        # Recreate the container when the K version has changed underneath it.
        if [ "$(docker container inspect --format '{{.Config.Image}}' "$CONTAINER")" != "$IMAGE" ]; then
            echo ">> K version changed, recreating container $CONTAINER"
            docker rm -f "$CONTAINER" >/dev/null
        fi
    fi
    if ! docker container inspect "$CONTAINER" >/dev/null 2>&1; then
        echo ">> creating container $CONTAINER"
        docker run --platform linux/amd64 --name "$CONTAINER" --detach --interactive --tty \
            --volume "$ROOT:$IN_CONTAINER_ROOT" \
            --volume "$CACHE_MOUNT:/home/user/.cache" \
            --workdir "$IN_CONTAINER_ROOT" \
            "$IMAGE" bash >/dev/null
    elif [ "$(docker container inspect --format '{{.State.Running}}' "$CONTAINER")" != "true" ]; then
        docker start "$CONTAINER" >/dev/null
    fi
}

# Runs a command inside the container.
in_container() {
    local tty=()
    [ -t 0 ] && [ -t 1 ] && tty=(--interactive --tty)
    docker exec ${tty[@]+"${tty[@]}"} "$CONTAINER" "$@"
}
