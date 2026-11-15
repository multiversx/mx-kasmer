import sys
from pathlib import Path
from pyk.kdist import kdist
from pyk.ktool.krun import KRun
from kmultiversx import kasmer

test_dir = Path(sys.argv[1])
contract_wasms = [Path(p) for p in sys.argv[2:]]
krun = KRun(kdist.get('mx-semantics.llvm-kasmer'))
test_wasm = kasmer.load_wasm(next(test_dir.glob('output/*.wasm')))
endpoints = dict(kasmer.get_test_endpoints(test_dir))
contracts = kasmer.load_contract_wasms(contract_wasms)
sym_conf, subst = kasmer.deploy_test(krun, test_wasm, contracts)
print('deploy + init OK')
kasmer.generate_claims(krun, endpoints, sym_conf, subst, test_dir / 'generated_claims')
print('claims:', sorted(p.name for p in (test_dir / 'generated_claims').iterdir()))
