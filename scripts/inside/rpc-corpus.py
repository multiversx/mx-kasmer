"""Records a proof of the Python tooling against kore-rpc-booster, as reference data for the Rust
crates kore-rpc and kcfg. Run through scripts/rpc-corpus.sh.

The proof is `kasmer verify` on a claim checked in with the integration tests of kmxwasm
(the same run as `RunClaim`, with the booster). Output, in the directory given as the first
argument, one subdirectory per claim:

    <claim>/server.json      how the server was started: main module and the options after the
                             definition, without --module, --server-port and
                             --llvm-backend-library (they depend on the machine)
    <claim>/exchanges.jsonl  every JSON-RPC exchange, in order: {"request": ..., "response": ...}
                             as sent and received (one line each, parsed)
    <claim>/extends.jsonl    for every `extend_cterm` of pyk's explorer: the exchange of its
                             `execute` (index into exchanges.jsonl) and how pyk read the result:
                             a list of {"kind": "step"|"stuck"|"vacuous"|"branch"|"ndbranch"|
                             "abstract", ...} (depth and cut for steps, branch counts)
    <claim>/result.txt       Success, Failed or Exception, as `kasmer verify` reports it
    <claim>/kcfg/            the proof graph as pyk writes it (kcfg.json and nodes/, in KAST)
"""

from __future__ import annotations

import json
import shutil
import sys
from pathlib import Path
from typing import Any

import pyk.kore.rpc as rpc
from pyk.kcfg import kcfg as kcfg_module
from pyk.kcfg.explore import KCFGExplore

from kmxwasm.property import RunClaim

ROOT = Path(__file__).resolve().parents[2]
CLAIMS_DIR = ROOT / 'kasmer-multiversx/kmxwasm/src/tests/integration/data'
CLAIMS = ['test_call_add']

# The configurations are deep; json.dumps recurses.
sys.setrecursionlimit(1_000_000)


class Recorder:
    def __init__(self) -> None:
        self.exchanges: Any = None
        self.extends: Any = None
        self.count = 0
        self.server: dict[str, Any] | None = None

    def start(self, out: Path) -> None:
        self.exchanges = (out / 'exchanges.jsonl').open('w')
        self.extends = (out / 'extends.jsonl').open('w')
        self.count = 0
        self.server = None

    def stop(self) -> None:
        self.exchanges.close()
        self.extends.close()


REC = Recorder()


def patch() -> None:
    request = rpc.SingleSocketTransport._request

    def recording_request(self: rpc.SingleSocketTransport, req: str) -> str:
        resp = request(self, req)
        REC.exchanges.write(json.dumps({'request': json.loads(req), 'response': json.loads(resp)}) + '\n')
        REC.count += 1
        return resp

    rpc.SingleSocketTransport._request = recording_request  # type: ignore

    create_proc = rpc.KoreServer._create_proc

    def recording_create_proc(args: list[str], env: dict[str, str]) -> Any:
        REC.server = server_options(args)
        return create_proc(args, env)

    rpc.KoreServer._create_proc = staticmethod(recording_create_proc)  # type: ignore

    extend_cterm = KCFGExplore.extend_cterm

    def recording_extend_cterm(self: KCFGExplore, *args: Any, **kwargs: Any) -> Any:
        before = REC.count
        results = extend_cterm(self, *args, **kwargs)
        executes = REC.count - before
        REC.extends.write(
            json.dumps(
                {
                    'exchange': REC.count - 1 if executes else None,
                    'results': [describe(r) for r in results],
                }
            )
            + '\n'
        )
        return results

    KCFGExplore.extend_cterm = recording_extend_cterm  # type: ignore


def server_options(args: list[str]) -> dict[str, Any]:
    """`kore-rpc-booster <options> <definition> --module M --server-port P ... --llvm-backend-library L`
    (pyk puts the definition after the command, which here includes options)."""
    module = None
    options = []
    i = 1
    while i < len(args):
        a = args[i]
        if a.endswith('/definition.kore'):
            i += 1
        elif a == '--module':
            module = args[i + 1]
            i += 2
        elif a in ('--server-port', '--llvm-backend-library'):
            i += 2
        else:
            options.append(a)
            i += 1
    return {'command': Path(args[0]).name, 'module': module, 'options': options}


def describe(result: kcfg_module.KCFGExtendResult) -> dict[str, Any]:
    match result:
        case kcfg_module.Step(_, depth, _, rules, cut):
            return {'kind': 'step', 'depth': depth, 'cut': cut, 'rules': list(rules)}
        case kcfg_module.Stuck():
            return {'kind': 'stuck'}
        case kcfg_module.Vacuous():
            return {'kind': 'vacuous'}
        case kcfg_module.Branch(constraints, _):
            return {'kind': 'branch', 'branches': len(constraints)}
        case kcfg_module.NDBranch(cterms, _, rules):
            return {'kind': 'ndbranch', 'branches': len(cterms), 'rules': list(rules)}
        case kcfg_module.Abstract():
            return {'kind': 'abstract'}
    raise AssertionError(result)


def record(out: Path, claim: str) -> None:
    out = out / claim
    if out.exists():
        shutil.rmtree(out)
    out.mkdir(parents=True)
    REC.start(out)
    action = RunClaim(
        claim_path=CLAIMS_DIR / f'{claim}-spec.json',
        is_k=False,
        restart=False,
        booster=True,
        remove=[],
        run_node_id=None,
        depth=10000,
        iterations=10000,
        kcfg_path=out / 'kcfg',
        bug_report=None,
    )
    result = 'Success'
    try:
        action.run()
    except SystemExit:
        result = 'Failed'
    except BaseException as e:  # noqa: B036
        result = f'Exception: {e}'
    finally:
        REC.stop()
    (out / 'server.json').write_text(json.dumps(REC.server, indent=2) + '\n')
    (out / 'result.txt').write_text(result + '\n')
    print(f'{claim}: {result}, {REC.count} exchanges', flush=True)


def main() -> None:
    out = Path(sys.argv[1]).resolve()
    claims = sys.argv[2:] or CLAIMS
    patch()
    for claim in claims:
        record(out, claim)


if __name__ == '__main__':
    main()
