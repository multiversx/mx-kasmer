#!/usr/bin/env python3
"""One-off migration of checked-in KAST JSON files to wasm-semantics 0.1.157.

The repository holds K terms as JSON that contain parsed Wasm modules: the
runtime configurations (kdist/runtime/*.json, with the ESDT system contract)
and the claims of the integration tests. The .wasm files they were made from
are not in the repository, so the terms are rewritten instead of regenerated.

Changes in the Wasm syntax between 0.1.106 and 0.1.157 that apply:

  #block(type, body, meta)        ->  #block(type, body)
  #loop(type, body, meta)         ->  #loop(type, body)
  #if(type, then, else, meta)     ->  #if(type, then, else)
  #data(memidx, offset, bytes)    ->  #data(bytes, #active(memidx, offset))
  #export(name, idx)              ->  #export(name, #externIdxKIND(idx))
  <exports> name |-> idx          ->  <exports> name |-> #externIdxKIND(idx)

The old syntax did not record what kind of object an export refers to. It is
recovered from the name, which is sound for modules produced by the Rust
compiler: `memory` is the memory, `__data_end` and `__heap_base` are globals,
everything else is a function.

Usage: kast-wasm-semantics-0.1.157.py FILE.json...   (rewrites in place)
"""

import json
import sys
import threading
from collections import Counter

GLOBAL_EXPORTS = {'__data_end', '__heap_base'}
MEMORY_EXPORTS = {'memory'}

stats: Counter = Counter()


def app(label, *args):
    return {'node': 'KApply', 'label': {'node': 'KLabel', 'name': label, 'params': []}, 'args': list(args), 'arity': len(args), 'variable': False}


def extern_idx(name_term, idx):
    if idx.get('node') == 'KApply' and idx['label']['name'].startswith('aExternIdx'):
        return idx
    if name_term.get('node') != 'KToken':
        raise ValueError(f'export name is not a token: {name_term}')
    name = name_term['token'].strip('"')
    if name in MEMORY_EXPORTS:
        kind = 'Memory'
    elif name in GLOBAL_EXPORTS:
        kind = 'Global'
    else:
        kind = 'Func'
    stats[f'export as {kind}'] += 1
    return app(f'aExternIdx{kind}', idx)


def fix_exports_map(term):
    """Wraps the values of the map entries directly inside an <exports> cell."""
    if not isinstance(term, dict) or term.get('node') != 'KApply':
        return
    label = term['label']['name']
    if label == '_Map_':
        for arg in term['args']:
            fix_exports_map(arg)
    elif label == '_|->_':
        term['args'][1] = extern_idx(term['args'][0], term['args'][1])


def migrate(term):
    if isinstance(term, list):
        for item in term:
            migrate(item)
        return
    if not isinstance(term, dict):
        return
    for value in term.values():
        migrate(value)
    if term.get('node') != 'KApply':
        return
    label = term['label']['name']
    args = term['args']
    if label in ('aBlock', 'aLoop') and len(args) == 3:
        del args[2]
    elif label == 'aIf' and len(args) == 4:
        del args[3]
    elif label == 'aDataDefn' and len(args) == 3:
        term['args'] = [args[2], app('aDataModeActive', args[0], args[1])]
    elif label == 'aExportDefn':
        args[1] = extern_idx(args[0], args[1])
    elif label == '<exports>':
        fix_exports_map(args[0])
    else:
        return
    stats[label] += 1
    if 'arity' in term:
        term['arity'] = len(term['args'])


def main():
    for path in sys.argv[1:]:
        stats.clear()
        with open(path) as f:
            data = json.load(f)
        migrate(data)
        with open(path, 'w') as f:
            json.dump(data, f, sort_keys=True)
        print(path, dict(sorted(stats.items())))


if __name__ == '__main__':
    # Instruction lists are deeply nested terms.
    sys.setrecursionlimit(10**7)
    threading.stack_size(1024 * 1024 * 1024)
    thread = threading.Thread(target=main)
    thread.start()
    thread.join()
