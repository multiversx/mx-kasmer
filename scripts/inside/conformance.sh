#!/usr/bin/env bash
# Runs inside a container with an mx-sdk-rs checkout at /sdk. See scripts/conformance.sh.

source "$(dirname "${BASH_SOURCE[0]}")/env.sh"
activate_venv
cd /sdk

scenarios=()
for arg in "$@"; do
    case "$arg" in
        *.test|*.txt) while IFS= read -r line; do
                          [ -n "$line" ] && scenarios+=("${line#deps/mx-sdk-rs/}")
                      done < "$arg" ;;
        *) scenarios+=("${arg#deps/mx-sdk-rs/}") ;;
    esac
done

passed=0; failed=()
for s in "${scenarios[@]}"; do
    # Scenarios of this repository (tests/custom-scenarios/...) are listed next to the others.
    file=$s
    [ -f "$file" ] || file="$ROOT/mx-semantics/$s"
    if [ ! -f "$file" ]; then
        echo "MISSING $s"; failed+=("$s"); continue
    fi
    if out=$(mandos "$file" 2>&1); then
        passed=$((passed + 1))
    else
        echo "FAILED  $s"
        # What the scenario got stuck on: the first item of the k cell, if there is a dump.
        dump=$(echo "$out" | sed -n 's/^See //p' | tail -1 || true)
        stuck=$(cat "$dump"/*_full.pretty.k 2>/dev/null | sed -n '/<k>/,/~>/p' | sed -n 2p | cut -c1-140 || true)
        echo "        ${stuck:-$(echo "$out" | tail -1)}"
        failed+=("$s")
    fi
done
echo "conformance: ${#scenarios[@]} scenarios, $passed passed, ${#failed[@]} failed"
[ ${#failed[@]} -eq 0 ]
