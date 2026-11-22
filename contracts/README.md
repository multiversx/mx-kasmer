# Test contracts

Compiled contracts used by the tests, with their scenarios. The sources are not kept here; each
contract directory has a `README.md` naming the repository and commit it was built from.

The layout mirrors `contracts/` in [mx-sdk-rs](https://github.com/multiversx/mx-sdk-rs), so the
relative paths in scenarios and in `kasmer.json` files are the same in both repositories.

| Directory | Contents |
|---|---|
| `examples/` | Contracts under test: `adder`, `crowdfunding`, `multisig` |
| `parametric-tests/*-pt*` | Parametric test contracts, each with a `kasmer.json` naming the contracts it tests |
| `parametric-tests/` (others) | Helper contracts of the scenario tests: `myadder` and `addercaller`, `caller` and `callee`, `magic-adder` (contract under test of `adder-pt-magic`, with a planted bug) |
| `unported/` | Test contracts of the original case studies whose contracts under test live in other repositories. Still Rust sources, not built; see the README there |

Each contract directory holds:

- `output/<name>.wasm` and `output/<name>.abi.json`, built with
  `sc-meta all build --wasm-symbols --no-wasm-opt`;
- `scenarios/`, where the contract has scenarios run by `scripts/test.sh`;
- `kasmer.json`, for parametric test contracts;
- `README.md`: source repository and commit.

## Rebuilding

Change the sources in mx-sdk-rs and commit there, then run from the mx-sdk-rs root (needs
`sc-meta`):

```sh
./kasmer-test-build-copy.sh <path to mx-kasmer>
```

The script, the counterpart of `vm-test-build-copy.sh` for the Go VM, builds each contract and
replaces its directory here. It warns about contracts built from uncommitted changes and says so
in their README. The list of contracts is in the script.
