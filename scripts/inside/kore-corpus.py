"""Writes KORE produced by pyk and by the LLVM interpreter, as reference data for the Rust KORE
crate (crates/kore/tests/conformance.rs). Run through scripts/kore-corpus.sh.

Output, in the directory given as the only argument:

    definitions/<name>.kore         definition.kore of a compiled target, as the compiler wrote it
    definitions/<name>.pyk.kore     the same, parsed and printed by pyk
    axioms/<name>.kore              one axiom pattern per line, printed by pyk
    axioms/<name>.jsonl             the same patterns as KORE JSON, one per line
    configs/<name>.kore             a configuration printed by pyk
    configs/<name>.json             the same as KORE JSON (with the format/version wrapper)
    configs/<name>.interpreter.kore the raw output of the LLVM interpreter, where there is one
    wasm/index.tsv                  for each .wasm file under contracts/: its path, used as the
                                    module's file name, and the name of the file with its term
    wasm/<name>.kore                the module as a KORE term of sort ModuleDecl: pykwasm's
                                    wasm2kast, then pyk's kast_to_kore (crates/kwasm)
    wasm/wat/<name>.wasm            crates/kwasm/tests/wat/<name>.wat compiled with wat2wasm
    wasm/wat/<name>.kore            its module, converted as above, without a file name
    kasmer/runtime.kore             the runtime configuration (with the ESDT system contract), as
                                    kmultiversx.runtime computes it from
                                    contracts/esdt-system-sc; the projects below are deployed on it
                                    rather than on the precomputed kdist/runtime/llvm-kasmer.json
    kasmer/<project>/template.kore  a test project of contracts/parametric-tests deployed as
                                    `kasmer fuzz` deploys it, with the variable K_STEPS in the
                                    <k> cell: the template the fuzzer fills (crates/kasmer)
    kasmer/<project>/calls.jsonl    calls of its test endpoints with fixed arguments: endpoint,
                                    arguments, the steps put in <k> (KORE text) and the exit code
                                    of the interpreter on the filled template

pykwasm is corrected before converting: it gives i64.extend8_s, i64.extend16_s and
i64.extend32_s the type i32 (kwasm_ast.py), crates/kwasm gives them i64.
"""

from __future__ import annotations

import json
import os
import subprocess
import sys
from pathlib import Path

from pyk.kast.inner import KApply, KSequence, KSort, KVariable, Subst
from pyk.kast.manip import split_config_from
from pyk.kast.prelude.utils import token
from pyk.kdist import kdist
from pyk.kore.parser import KoreParser
from pyk.kore.syntax import Axiom, Pattern
from pyk.ktool.krun import KRun

import pykwasm.binary.instructions
import pykwasm.kwasm_ast as wast
from pykwasm.wasm2kast import wasm2kast

from kmultiversx import kasmer, scenario
from kmultiversx.runtime import load_runtime
from kmultiversx.scenario import KMap
from kmultiversx.utils import GENERATED_TOP_CELL, llvm_interpret_raw, load_wasm, read_kasmer_runtime

# The configurations are deep (cons lists, nested sequences); json.dumps recurses.
sys.setrecursionlimit(1_000_000)

ROOT = Path(__file__).resolve().parents[2]

DEFINITIONS = {
    'mx-semantics-llvm-kasmer': 'mx-semantics.llvm-kasmer',
    'mxwasm-semantics-haskell': 'mxwasm-semantics.haskell',
}

# Contracts deployed into the runtime configuration, to get configurations with real Wasm modules.
CONTRACTS = {
    'adder': ROOT / 'contracts/examples/adder/output/adder.wasm',
    'multisig': ROOT / 'contracts/examples/multisig/output/multisig.wasm',
}

ADDRESS = '0x' + '00' * 31 + '01'


def kore_json(pattern: Pattern) -> str:
    return json.dumps({'format': 'KORE', 'version': 1, 'term': pattern.dict}, sort_keys=True)


def write_definitions(out: Path) -> None:
    d = out / 'definitions'
    a = out / 'axioms'
    d.mkdir(parents=True, exist_ok=True)
    a.mkdir(parents=True, exist_ok=True)
    for name, target in DEFINITIONS.items():
        text = (kdist.get(target) / 'definition.kore').read_text()
        definition = KoreParser(text).definition()
        (d / f'{name}.kore').write_text(text)
        (d / f'{name}.pyk.kore').write_text(definition.text)
        axioms = [s.pattern for m in definition.modules for s in m.sentences if isinstance(s, Axiom)]
        with (a / f'{name}.kore').open('w') as kore, (a / f'{name}.jsonl').open('w') as jsonl:
            for pattern in axioms:
                kore.write(pattern.text + '\n')
                jsonl.write(json.dumps(pattern.dict, sort_keys=True) + '\n')
        print(f'{name}: {len(definition.modules)} modules, {len(axioms)} axioms')


def write_config(out: Path, name: str, pattern: Pattern, interpreter_output: str | None = None) -> None:
    c = out / 'configs'
    c.mkdir(parents=True, exist_ok=True)
    (c / f'{name}.kore').write_text(pattern.text)
    (c / f'{name}.json').write_text(kore_json(pattern))
    if interpreter_output is not None:
        (c / f'{name}.interpreter.kore').write_text(interpreter_output)
    print(f'config {name}: {len(pattern.text)} characters')


def write_configs(out: Path) -> None:
    definition_dir = kdist.get('mx-semantics.llvm-kasmer')
    krun = KRun(definition_dir)

    runtime = read_kasmer_runtime()
    write_config(out, 'runtime-kasmer', krun.kast_to_kore(runtime, sort=GENERATED_TOP_CELL))

    for name, wasm_path in CONTRACTS.items():
        conf, subst = split_config_from(runtime)
        subst['K_CELL'] = KApply(
            'setAccount',
            [
                token(scenario.mandos_string_to_bytes(ADDRESS)),
                token(0),
                token(0),
                load_wasm(wasm_path),
                token(b''),
                KMap([]),
            ],
        )
        before = krun.kast_to_kore(Subst(subst)(conf), sort=GENERATED_TOP_CELL)
        write_config(out, f'deploy-{name}-before', before)

        proc = llvm_interpret_raw(definition_dir, before.text)
        if proc.returncode != 0:
            raise RuntimeError(f'interpreter failed on {name}: {proc.returncode}\n{proc.stderr}')
        after = KoreParser(proc.stdout).pattern()
        write_config(out, f'deploy-{name}-after', after, interpreter_output=proc.stdout)


def fix_pykwasm() -> None:
    """Give the i64 sign-extension instructions the type i64. The instruction parser refers to
    the constants through the module, so replacing them there is enough."""
    for name, op in [('I64_EXTEND8_s', 'aExtend8_s'), ('I64_EXTEND16_s', 'aExtend16_s'), ('I64_EXTEND32_s', 'aExtend32_s')]:
        setattr(wast, name, KApply('aExtendS', [wast.i64, KApply(op, [])]))
    assert pykwasm.binary.instructions.wast is wast


def write_wasm(out: Path) -> None:
    fix_pykwasm()
    w = out / 'wasm'
    w.mkdir(parents=True, exist_ok=True)
    krun = KRun(kdist.get('mx-semantics.llvm-kasmer'))
    # Relative paths, so that the file name stored in the module does not depend on where the
    # repository is.
    os.chdir(ROOT)
    index = []
    for path in sorted(Path('contracts').glob('**/output/*.wasm')):
        name = str(path).replace('/', '__').removesuffix('.wasm')
        pattern = krun.kast_to_kore(load_wasm(path), sort=KSort('ModuleDecl'))
        (w / f'{name}.kore').write_text(pattern.text)
        index.append(f'{path}\t{name}.kore\n')
    (w / 'index.tsv').write_text(''.join(index))
    print(f'wasm: {len(index)} modules')

    wat = w / 'wat'
    wat.mkdir(exist_ok=True)
    wat_files = sorted(Path('crates/kwasm/tests/wat').glob('*.wat'))
    for path in wat_files:
        binary = wat / f'{path.stem}.wasm'
        subprocess.run(['wat2wasm', str(path), '-o', str(binary)], check=True)
        with binary.open('rb') as f:
            module = wasm2kast(f)
        pattern = krun.kast_to_kore(module, sort=KSort('ModuleDecl'))
        (wat / f'{path.stem}.kore').write_text(pattern.text)
    print(f'wat: {len(wat_files)} modules')


# Test projects run through the fuzzing flow of kmultiversx.kasmer, with fixed arguments.
KASMER_PROJECTS = ['adder-pt', 'adder-pt-fail', 'crowdfunding-pt', 'multisig-pt']

# Arguments per ABI type. Each call passes the same value for every argument.
KASMER_ARGUMENTS = {
    'BigUint': ['0', '1', '255', '256', '1000000', str(2**64), str(2**256 - 1)],
    'u32': ['0', '1', '4294967295'],
    'u64': ['0', '1', '18446744073709551615'],
}


def kasmer_steps(krun: KRun, endpoint: str, args: tuple[str, ...]) -> Pattern:
    """The <k> cell of one fuzzing run, as `_make_k_steps` in kmultiversx.kasmer builds it."""
    step = {
        'tx': {
            'from': kasmer.ROOT_ACCT_ADDR,
            'to': kasmer.TEST_SC_ADDR,
            'function': endpoint,
            'value': '0',
            'arguments': args,
            'gasLimit': '5,000,000,000',
            'gasPrice': '0',
        },
        'expect': {'status': '0'},
    }
    steps = [kasmer.set_exit_code(1)] + scenario.get_steps_sc_call(step) + [kasmer.set_exit_code(0)]
    return krun.kast_to_kore(KSequence(steps))


def contract_wasm_path(test_dir: Path, contract: str | dict) -> Path:
    """As kmxwasm.kasmer.utils.read_contract_target, without making the path absolute."""
    if isinstance(contract, str):
        contract = {'path': contract}
    directory = Path(os.path.normpath(test_dir / contract['path']))
    if 'name' in contract:
        return directory / 'output' / f"{contract['name']}.wasm"
    (path,) = directory.glob('output/*.wasm')
    return path


def write_kasmer(out: Path) -> None:
    """Paths are relative to the repository root (the current directory, see write_wasm), so that
    the file names stored in the modules do not depend on where the repository is."""
    definition_dir = kdist.get('mx-semantics.llvm-kasmer')
    krun = KRun(definition_dir)
    runtime = load_runtime(krun, load_wasm(Path('contracts/esdt-system-sc/output/esdt-system-sc.wasm')))
    (out / 'kasmer').mkdir(parents=True, exist_ok=True)
    (out / 'kasmer' / 'runtime.kore').write_text(krun.kast_to_kore(runtime, sort=GENERATED_TOP_CELL).text)
    # deploy_test starts from the precomputed runtime; start from this one instead.
    kasmer.read_kasmer_runtime = lambda: runtime
    for name in KASMER_PROJECTS:
        test_dir = Path('contracts/parametric-tests') / name
        (test_wasm_path,) = test_dir.glob('output/*.wasm')
        project = json.loads((test_dir / 'kasmer.json').read_text())
        contract_paths = [contract_wasm_path(test_dir, c) for c in project['contracts']]
        sym_conf, init_subst = kasmer.deploy_test(
            krun, load_wasm(test_wasm_path), kasmer.load_contract_wasms(contract_paths)
        )
        subst = dict(init_subst)
        subst['K_CELL'] = KVariable('K_STEPS')
        template = krun.kast_to_kore(Subst(subst)(sym_conf))
        if template.symbol == 'inj':  # as in kasmer.run_concrete
            template = template.args[0]
        template_text = template.text

        d = out / 'kasmer' / name
        d.mkdir(parents=True, exist_ok=True)
        (d / 'template.kore').write_text(template_text)
        var = "VarK'Unds'STEPS : SortK{}"
        assert template_text.count(var) == 1, var

        calls = []
        for endpoint, types in kasmer.get_test_endpoints(test_dir).items():
            for i in range(max((len(KASMER_ARGUMENTS[t]) for t in types), default=1)):
                args = tuple(KASMER_ARGUMENTS[t][min(i, len(KASMER_ARGUMENTS[t]) - 1)] for t in types)
                steps = kasmer_steps(krun, endpoint, args).text
                proc = llvm_interpret_raw(definition_dir, template_text.replace(var, steps))
                calls.append({'endpoint': endpoint, 'args': list(args), 'steps': steps, 'exit_code': proc.returncode})
                print(f'kasmer {name} {endpoint}{args}: exit code {proc.returncode}')
        (d / 'calls.jsonl').write_text(''.join(json.dumps(c, sort_keys=True) + '\n' for c in calls))


def main() -> None:
    out = Path(sys.argv[1]).resolve()
    write_definitions(out)
    write_configs(out)
    write_wasm(out)
    write_kasmer(out)


if __name__ == '__main__':
    main()
