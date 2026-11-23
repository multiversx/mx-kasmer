#!/usr/bin/env bash
# Makes kore-rpc-booster with the symbolic definition (mxwasm-semantics.haskell, with
# mxwasm-semantics.llvm-library for concrete evaluation) usable from the host, for the Rust
# tooling. Writes a server directory (see crates/kore-rpc/src/server.rs):
#
#   .build/kore-rpc/definition.kore   copied from the compiled definition in the container
#   .build/kore-rpc/server            starts the booster in the container
#   .build/kore-rpc/connect           forwards a connection to it through `docker exec`
#   .build/kore-rpc/inside/           the scripts that run in the container
#
# Pass the directory to the Rust tools (environment variable KASMER_RPC_SERVER). The server stops
# when the standard input of `server` closes, also when the host process is killed.
#
# Run again after the definition is rebuilt (scripts/build.sh).

source "$(dirname "${BASH_SOURCE[0]}")/lib.sh"

OUT="$ROOT/.build/kore-rpc"

ensure_container
dirs=$(in_container bash -c 'source scripts/inside/env.sh; activate_venv
    python -c "from pyk.kdist import kdist
print(kdist.which(\"mxwasm-semantics.haskell\"))
print(kdist.which(\"mxwasm-semantics.llvm-library\"))"')
dirs=${dirs//$'\r'/}
haskell=$(sed -n 1p <<< "$dirs")
library=$(sed -n 2p <<< "$dirs")
for f in "$haskell/definition.kore" "$library/interpreter.so"; do
    if ! in_container test -f "$f"; then
        echo "!! $f not found; build the definitions first: scripts/build.sh" >&2
        exit 1
    fi
done
module=$(in_container cat "$haskell/mainModule.txt")
module=${module%$'\r'}

mkdir -p "$OUT/inside"
rel_out=${OUT#"$ROOT"/}
inside="$IN_CONTAINER_ROOT/$rel_out/inside"
in_container cp "$haskell/definition.kore" "$IN_CONTAINER_ROOT/$rel_out/definition.kore"

cat > "$OUT/inside/server" <<SCRIPT
#!/usr/bin/env bash
# Written by scripts/kore-rpc-host.sh. Runs in the container: kore-rpc-booster on
#   $haskell
#   $library
# Called as: server --server-port <port> [options...]. Stops the server when stdin closes.
kore-rpc-booster "$haskell/definition.kore" --module "$module" \\
    --llvm-backend-library "$library/interpreter.so" "\$@" < /dev/null &
pid=\$!
# A background job reads /dev/null unless told otherwise: give it our stdin explicitly.
exec 3<&0
{ cat <&3 > /dev/null; kill "\$pid" 2> /dev/null; } &
wait "\$pid"
SCRIPT

cat > "$OUT/inside/connect" <<'SCRIPT'
#!/usr/bin/env bash
# Written by scripts/kore-rpc-host.sh. Runs in the container: forwards stdin and stdout to
# 127.0.0.1:<port>. Called as: connect <port>
exec 3<> "/dev/tcp/127.0.0.1/$1" || exit 1
cat <&3 &
reader=$!
cat >&3
kill "$reader" 2> /dev/null
SCRIPT

for script in server connect; do
    cat > "$OUT/$script" <<SCRIPT
#!/usr/bin/env bash
# Written by scripts/kore-rpc-host.sh: runs inside/$script in the container $CONTAINER.
exec docker exec -i "$CONTAINER" "$inside/$script" "\$@"
SCRIPT
done
chmod +x "$OUT/server" "$OUT/connect" "$OUT/inside/server" "$OUT/inside/connect"
echo "$OUT"
