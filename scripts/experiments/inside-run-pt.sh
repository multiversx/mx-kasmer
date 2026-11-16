#!/usr/bin/env bash
# Runs inside the container. See run-pt.sh.

source "$(dirname "${BASH_SOURCE[0]}")/../inside/env.sh"
activate_venv

RUN="$ROOT/$1"
EXP="$ROOT/scripts/experiments"
cd "$RUN"

echo ">> fuzzing (concrete, LLVM backend)"
status=0
python3 "$EXP/fuzz_pt.py" test contracts/output/*.wasm > fuzz.log 2>&1 || status=1
grep -E "^(deploy|Testing|Passed|FAILED|fuzzing:)" fuzz.log || true
echo "   full log: .pt-runs/$(basename "$RUN")/fuzz.log"

echo ">> generating claims"
kasmer -C test claims 2>&1 | grep -vE 'pyk\.'

cd test
for claim in generated_claims/*-spec.json; do
    t=$(basename "$claim" -spec.json)
    echo ">> verifying $t (symbolic, booster backend); log: .pt-runs/$(basename "$RUN")/verify-$t.log"
    if kasmer verify "$t" --booster > "../verify-$t.log" 2>&1; then
        echo "   Success"
    else
        echo "   FAILED: $(grep -m1 -E '^(Failed|Exception)' "../verify-$t.log" || echo 'see log')"
        status=1
    fi
done
exit $status
