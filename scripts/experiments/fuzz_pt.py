"""Fuzz every test_* endpoint of a parametric test contract on the LLVM backend.

usage: fuzz_pt.py TEST_DIR CONTRACT.wasm...

Reports each endpoint separately; exits with status 1 if any endpoint fails.
"""

import sys
from pathlib import Path

from kmultiversx import kasmer
from pyk.kdist import kdist
from pyk.ktool.krun import KRun

test_dir = Path(sys.argv[1])
contract_wasms = [Path(p) for p in sys.argv[2:]]
krun = KRun(kdist.get('mx-semantics.llvm-kasmer'))
test_wasm = kasmer.load_wasm(next(test_dir.glob('output/*.wasm')))
endpoints = dict(kasmer.get_test_endpoints(test_dir))
contracts = kasmer.load_contract_wasms(contract_wasms)
sym_conf, subst = kasmer.deploy_test(krun, test_wasm, contracts)
print('deploy + init OK')

failed = []
for name, arg_types in endpoints.items():
    try:
        kasmer.run_concrete(krun, {name: arg_types}, sym_conf, subst, verbose=False)
    except (ValueError, AssertionError) as err:
        # Depending on the pyk version, a failing run surfaces as a ValueError carrying the
        # VM output or as a bare AssertionError from KFuzz. Either way Hypothesis has printed
        # the falsifying example on stderr.
        failed.append(name)
        reason = [line.strip() for line in str(err).splitlines() if 'vm output' in line]
        print(f'FAILED {name!r}', *reason)

print(f'fuzzing: {len(endpoints) - len(failed)} passed, {len(failed)} failed')
sys.exit(1 if failed else 0)
