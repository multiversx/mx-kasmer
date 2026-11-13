# Common definitions for the host-side scripts. Source this file, do not run it.
#
# The repository still consists of two halves that need different K versions,
# so each half gets its own image and its own long-lived container:
#
#   half               K version file                    container
#   mx-semantics       mx-semantics/deps/k_release       mx-kasmer-mx-semantics
#   kasmer-multiversx  kasmer-multiversx/deps/k_release  mx-kasmer-kasmer-multiversx
#
# The repository is bind-mounted into the container, so edits on the host are
# visible immediately. Build products (compiled K definitions, Python virtual
# environments) live in a named Docker volume per half and survive the container.

set -euo pipefail

ROOT=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
IN_CONTAINER_ROOT=/home/user/mx-kasmer
MIN_DOCKER_MEM_GB=16

usage_half() {
    echo "usage: $(basename "$0") <mx-semantics|kasmer-multiversx> $*" >&2
    exit 2
}

select_half() {
    case "${1:-}" in
        mx-semantics|kasmer-multiversx) HALF=$1 ;;
        *) return 1 ;;
    esac
    K_VERSION=$(cat "$ROOT/$HALF/deps/k_release")
    IMAGE="mx-kasmer-dev:k$K_VERSION"
    CONTAINER="mx-kasmer-$HALF"
    VOLUME="mx-kasmer-cache-$HALF"
}

ensure_image() {
    if ! docker image inspect "$IMAGE" >/dev/null 2>&1; then
        echo ">> building image $IMAGE"
        docker build --platform linux/amd64 --build-arg K_VERSION="$K_VERSION" -t "$IMAGE" "$ROOT"
    fi
}

check_docker_memory() {
    local bytes gb
    bytes=$(docker info --format '{{.MemTotal}}')
    gb=$((bytes / 1024 / 1024 / 1024))
    if [ "$gb" -lt "$MIN_DOCKER_MEM_GB" ]; then
        echo "!! Docker has ${gb} GB of memory. Compiling the semantics needs more than 8 GB;" >&2
        echo "!! give Docker at least ${MIN_DOCKER_MEM_GB} GB (Docker Desktop: Settings > Resources)." >&2
        exit 1
    fi
}

ensure_container() {
    ensure_image
    if ! docker container inspect "$CONTAINER" >/dev/null 2>&1; then
        echo ">> creating container $CONTAINER"
        docker run --platform linux/amd64 --name "$CONTAINER" --detach --interactive --tty \
            --volume "$ROOT:$IN_CONTAINER_ROOT" \
            --volume "$VOLUME:/home/user/.cache" \
            --workdir "$IN_CONTAINER_ROOT" \
            "$IMAGE" bash >/dev/null
    elif [ "$(docker container inspect --format '{{.State.Running}}' "$CONTAINER")" != "true" ]; then
        docker start "$CONTAINER" >/dev/null
    fi
}

# Runs a command inside the container of the selected half.
in_container() {
    local tty=()
    [ -t 0 ] && [ -t 1 ] && tty=(--interactive --tty)
    docker exec ${tty[@]+"${tty[@]}"} --env HALF="$HALF" "$CONTAINER" "$@"
}
