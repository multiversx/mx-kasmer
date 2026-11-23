#!/usr/bin/env bash
# Makes the LLVM definition mx-semantics.llvm-kasmer usable from the host, for the Rust tooling:
#
#   .build/llvm-kasmer/definition.kore   copied from the compiled definition in the container
#   .build/llvm-kasmer/interpreter       runs the definition's interpreter in the container
#
# Pass the directory to the Rust tools (--definition .build/llvm-kasmer, or the environment
# variable KASMER_LLVM_DEFINITION). On a host where the K backends run natively, the compiled
# definition directory itself can be used instead.
#
# Run again after the definition is rebuilt (scripts/build.sh).

source "$(dirname "${BASH_SOURCE[0]}")/lib.sh"

OUT="$ROOT/.build/llvm-kasmer"

ensure_container
dir=$(in_container bash -c 'source scripts/inside/env.sh; activate_venv
    python -c "from pyk.kdist import kdist; print(kdist.which(\"mx-semantics.llvm-kasmer\"))"')
dir=${dir%$'\r'}
if ! in_container test -x "$dir/interpreter"; then
    echo "!! $dir/interpreter not found; build the definition first: scripts/build.sh" >&2
    exit 1
fi

mkdir -p "$OUT"
rel_out=${OUT#"$ROOT"/}
in_container cp "$dir/definition.kore" "$IN_CONTAINER_ROOT/$rel_out/definition.kore"
cat > "$OUT/interpreter" <<WRAPPER
#!/usr/bin/env bash
# Written by scripts/llvm-kasmer-host.sh: the interpreter of $dir in the container $CONTAINER.
# Called as: interpreter /dev/stdin <depth> /dev/stdout
exec docker exec -i "$CONTAINER" "$dir/interpreter" "\$@"
WRAPPER
chmod +x "$OUT/interpreter"
echo "$OUT"
