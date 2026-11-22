# Unported test contracts

Test contracts of the original case studies, kept as Rust sources as they were imported. They test
contracts that are not in mx-sdk-rs, and have not been ported to the current framework yet:

| Test contract | Contract under test |
|---|---|
| `test_coindrip` | [CoinDrip-finance/coindrip-protocol-sc](https://github.com/CoinDrip-finance/coindrip-protocol-sc), last pinned at `3934e6e837e94cfa143c18fd6292fbe982f976cf` |
| `test_pair` | `dex/pair` in [multiversx/mx-exchange-sc](https://github.com/multiversx/mx-exchange-sc), last pinned at `ee2d4645bd13c0f22f668a72bbc1b883753b6aee` |

They do not build as they are: they use `multiversx-sc` 0.50.1 and a `testapi` crate that no
longer exists (the test API is now `self.test_raw()` in the framework), and their `kasmer.json`
and scenarios point into the removed `deps/` submodules.

The claims generated from them by the original project are still used as test data in
`kasmer-multiversx/kmxwasm/src/tests/integration/data/`.
