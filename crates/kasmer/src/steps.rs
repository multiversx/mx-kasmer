//! Terms of the MultiversX semantics used to set up and run tests: scenario steps and the values in
//! them. Built by K label through [`Symbols`], as pyk's `kast_to_kore` builds them from the KAST of
//! `kmultiversx.kasmer`, so that the KORE is the same.

use kore::symbols::{Symbols, Term};
use kore::{Pattern, Sort};

use crate::Error;

pub struct Builder<'a> {
    symbols: &'a Symbols,
}

type R<T> = Result<T, Error>;

fn k_item() -> Sort {
    Sort::app0("SortKItem")
}

impl<'a> Builder<'a> {
    pub fn new(symbols: &'a Symbols) -> Builder<'a> {
        Builder { symbols }
    }

    pub fn symbols(&self) -> &'a Symbols {
        self.symbols
    }

    pub fn app(&self, label: &str, args: Vec<Term>) -> R<Term> {
        Ok(self.symbols.apply(label, args)?)
    }

    pub fn int(&self, value: impl ToString) -> Term {
        Term::dv(Sort::app0("SortInt"), value.to_string())
    }

    /// K `Bytes`: in KORE, a string with one character per byte.
    pub fn bytes(&self, bs: &[u8]) -> Term {
        Term::dv(
            Sort::app0("SortBytes"),
            bs.iter().map(|&b| char::from(b)).collect::<String>(),
        )
    }

    /// A Wasm name, such as an endpoint name: the token is the name between double quotes.
    pub fn wasm_string(&self, s: &str) -> Term {
        Term::dv(Sort::app0("SortWasmStringToken"), format!("\"{s}\""))
    }

    /// A K sequence: `kseq{}(inj{S, SortKItem{}}(item), ...)`, ending in `.K`.
    pub fn kseq(&self, items: Vec<Term>) -> Term {
        let mut seq = Pattern::app("dotk", vec![], vec![]);
        for item in items.into_iter().rev() {
            seq = Pattern::app("kseq", vec![], vec![item.into_sort(&k_item()), seq]);
        }
        Term {
            pattern: seq,
            sort: Sort::app0("SortK"),
        }
    }

    /// A `Map`, as pyk's `map_of` builds it: items joined by `_Map_`, nested to the right.
    pub fn map(&self, items: Vec<(Term, Term)>) -> R<Term> {
        let mut items = items
            .into_iter()
            .map(|(k, v)| self.app("_|->_", vec![k, v]))
            .collect::<R<Vec<_>>>()?;
        let Some(mut map) = items.pop() else {
            return self.app(".Map", vec![]);
        };
        while let Some(item) = items.pop() {
            map = self.app("_Map_", vec![item, map])?;
        }
        Ok(map)
    }

    /// A `ListBytes` of `wrapBytes` items, as `kmultiversx.scenario.ListBytes`.
    pub fn list_bytes<B: AsRef<[u8]>>(&self, items: &[B]) -> R<Term> {
        self.list_bytes_of(items.iter().map(|i| self.bytes(i.as_ref())).collect())
    }

    /// A `ListBytes` of `wrapBytes` items, from terms of sort `Bytes`.
    pub fn list_bytes_of(&self, items: Vec<Term>) -> R<Term> {
        let mut list = self.app(".ListBytes", vec![])?;
        for item in items.into_iter().rev() {
            let wrapped = self.app("wrapBytes", vec![item])?;
            let item = self.app("ListBytesItem", vec![wrapped])?;
            list = self.app("_ListBytes_", vec![item, list])?;
        }
        Ok(list)
    }

    pub fn set_exit_code(&self, code: i64) -> R<Term> {
        self.app("setExitCode", vec![self.int(code)])
    }

    /// `setAccount(address, nonce, balance, code, owner, storage)`; `code` is `.Code` if `None`.
    pub fn set_account(
        &self,
        address: &[u8],
        nonce: u64,
        balance: u64,
        code: Option<Term>,
        owner: &[u8],
        storage: Term,
    ) -> R<Term> {
        let code = match code {
            Some(c) => c,
            None => self.app(".Code", vec![])?,
        };
        self.app(
            "setAccount",
            vec![
                self.bytes(address),
                self.int(nonce),
                self.int(balance),
                code,
                self.bytes(owner),
                storage,
            ],
        )
    }

    /// `newAddress(creator, nonce, new)`: the address that `creator` gets when it deploys with
    /// `nonce`.
    pub fn new_address(&self, creator: &[u8], nonce: u64, new: &[u8]) -> R<Term> {
        self.app(
            "newAddress",
            vec![self.bytes(creator), self.int(nonce), self.bytes(new)],
        )
    }

    pub fn deploy_tx<B: AsRef<[u8]>>(
        &self,
        from: &[u8],
        value: u64,
        module: Term,
        args: &[B],
        gas: u64,
        gas_price: u64,
    ) -> R<Term> {
        self.app(
            "deployTx",
            vec![
                self.bytes(from),
                self.int(value),
                module,
                self.list_bytes(args)?,
                self.int(gas),
                self.int(gas_price),
            ],
        )
    }

    /// `callTx(from, to, value, esdtValue, function, args, gasLimit, gasPrice)`, with no ESDT
    /// transfers. One parameter per argument of the step.
    #[allow(clippy::too_many_arguments)]
    pub fn call_tx<B: AsRef<[u8]>>(
        &self,
        from: &[u8],
        to: &[u8],
        value: u64,
        function: &str,
        args: &[B],
        gas: u64,
        gas_price: u64,
    ) -> R<Term> {
        let args = self.list_bytes(args)?;
        self.call_tx_with(from, to, value, function, args, gas, gas_price)
    }

    /// `callTx` with the arguments given as a term of sort `ListBytes`, such as symbolic ones.
    #[allow(clippy::too_many_arguments)]
    pub fn call_tx_with(
        &self,
        from: &[u8],
        to: &[u8],
        value: u64,
        function: &str,
        args: Term,
        gas: u64,
        gas_price: u64,
    ) -> R<Term> {
        self.app(
            "callTx",
            vec![
                self.bytes(from),
                self.bytes(to),
                self.int(value),
                self.app(".List", vec![])?,
                self.wasm_string(function),
                args,
                self.int(gas),
                self.int(gas_price),
            ],
        )
    }

    /// `checkExpectStatus(OK)`: the last transaction succeeded.
    pub fn check_expect_ok(&self) -> R<Term> {
        self.app("checkExpectStatus", vec![self.app("OK", vec![])?])
    }
}
