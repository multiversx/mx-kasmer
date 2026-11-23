//! The claim that a test endpoint passes for all arguments, as `kmultiversx.kasmer.generate_claim`
//! builds it and pyk's `KCFG.from_claim` turns it into the first two nodes of a proof.
//!
//! - **init**: the deployed configuration, with `<k>` calling the endpoint with symbolic arguments
//!   (`ARG_0`, ...), constrained to their type's range; `<vmOutput>` and `<logging>` arbitrary.
//! - **target**: the same configuration with `<k>` empty and the exit code 0, the call stack and the
//!   world state stack empty; accounts, logs, block information and the transaction count are
//!   whatever the call left (existential), as are `<vmOutput>` and `<logging>`.
//!
//! Variable names are pyk's: `_` in front of a variable that occurs once, `?` in front of one that
//! occurs only in the target (existential).

use kcfg::CTerm;
use kcfg::cterm::top_sort;
use kore::symbols::{Term, label_to_symbol};
use kore::{Pattern, Sort};

use crate::cells::{cell_symbol, set_cell};
use crate::fuzz::{ArgType, arg_types};
use crate::project::Endpoint;
use crate::setup::{ROOT_ACCOUNT, TEST_CONTRACT};
use crate::steps::Builder;
use crate::{Error, mandos};

/// The cells of the target that the call may change arbitrarily, besides `<vmOutput>` and
/// `<logging>`.
const FREE_IN_TARGET: [&str; 13] = [
    "accounts",
    "logs",
    "prevBlockTimestamp",
    "prevBlockNonce",
    "prevBlockRound",
    "prevBlockEpoch",
    "prevBlockRandomSeed",
    "curBlockTimestamp",
    "curBlockNonce",
    "curBlockRound",
    "curBlockEpoch",
    "curBlockRandomSeed",
    "txCount",
];

#[derive(Clone, Debug)]
pub struct Claim {
    pub endpoint: String,
    pub init: CTerm,
    pub target: CTerm,
}

/// The KORE name of a variable named `name` in K: `Var` and the name encoded as labels are.
pub fn var_name(name: &str) -> String {
    let symbol = label_to_symbol(name).expect("variable names are ASCII");
    format!("Var{}", &symbol["Lbl".len()..])
}

/// pyk's name of the variable for the content of a cell: `<prevBlockNonce>` is `PREVBLOCKNONCE_CELL`.
fn cell_var(cell: &str) -> String {
    format!("{}_CELL", cell.to_uppercase().replace('-', "_"))
}

/// A variable of the sort of the content of `<cell>`.
fn cell_content_var(b: &Builder, cell: &str, name: &str) -> Result<Pattern, Error> {
    let symbol = cell_symbol(cell);
    let sort = b
        .symbols()
        .signature(&symbol)
        .and_then(|s| s.params.first())
        .ok_or_else(|| Error::Project(format!("no cell <{cell}> in the definition")))?
        .clone();
    Ok(Pattern::evar(var_name(name), sort))
}

pub fn build(b: &Builder, deployed: &Pattern, endpoint: &Endpoint) -> Result<Claim, Error> {
    let types = arg_types(endpoint)?;
    let int = Sort::app0("SortInt");
    let args: Vec<Term> = (0..types.len())
        .map(|i| Term {
            pattern: Pattern::evar(var_name(&format!("ARG_{i}")), int.clone()),
            sort: int.clone(),
        })
        .collect();

    // As bytes: Int2Bytes(ARG, BE, Signed), as kmultiversx.kasmer.var_to_bytes.
    let bytes = args
        .iter()
        .map(|a| {
            b.app(
                "Int2BytesNoLen",
                vec![
                    a.clone(),
                    b.app("bigEndianBytes", vec![])?,
                    b.app("signedBytes", vec![])?,
                ],
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    let steps = b.kseq(vec![
        b.set_exit_code(1)?,
        b.call_tx_with(
            &mandos::address(ROOT_ACCOUNT),
            &mandos::sc_address(TEST_CONTRACT),
            0,
            &endpoint.name,
            b.list_bytes_of(bytes)?,
            crate::fuzz::CALL_GAS,
            0,
        )?,
        b.check_expect_ok()?,
        b.set_exit_code(0)?,
    ]);

    let mut constraints = Vec::new();
    for (arg, t) in args.iter().zip(&types) {
        constraints.push(equals_true(b.app("_<=Int_", vec![b.int(0), arg.clone()])?));
        let max = match t {
            ArgType::BigUint => None,
            ArgType::U32 => Some(u32::MAX as u64),
            ArgType::U64 => Some(u64::MAX),
        };
        if let Some(max) = max {
            constraints.push(equals_true(
                b.app("_<=Int_", vec![arg.clone(), b.int(max)])?,
            ));
        }
    }

    // Cells common to both sides.
    let mut common = deployed.clone();
    set_cell(
        &mut common,
        "checkedAccounts",
        b.app(".Set", vec![])?.pattern,
    )?;
    set_cell(&mut common, "commands", dotk())?;
    set_cell(&mut common, "instrs", dotk())?;
    set_cell(&mut common, "callStack", b.app(".List", vec![])?.pattern)?;
    set_cell(
        &mut common,
        "interimStates",
        b.app(".List", vec![])?.pattern,
    )?;
    set_cell(&mut common, "exit-code", b.int(0).pattern)?;
    set_cell(
        &mut common,
        "prank",
        Pattern::dv(Sort::app0("SortBool"), "false"),
    )?;
    set_cell(&mut common, "wasmStore", b.app(".Map", vec![])?.pattern)?;

    let mut init = common.clone();
    set_cell(&mut init, "k", steps.pattern)?;
    set_cell(
        &mut init,
        "vmOutput",
        cell_content_var(b, "vmOutput", "_VMOUTPUT_CELL")?,
    )?;
    set_cell(
        &mut init,
        "logging",
        cell_content_var(b, "logging", "_LOGGING_CELL")?,
    )?;

    let mut target = common;
    set_cell(&mut target, "k", dotk())?;
    for cell in FREE_IN_TARGET {
        let name = format!("?_{}", cell_var(cell));
        set_cell(&mut target, cell, cell_content_var(b, cell, &name)?)?;
    }
    set_cell(
        &mut target,
        "vmOutput",
        cell_content_var(b, "vmOutput", "?_VMOUTPUT_CELL_R")?,
    )?;
    set_cell(
        &mut target,
        "logging",
        cell_content_var(b, "logging", "?_LOGGING_CELL_R")?,
    )?;

    Ok(Claim {
        endpoint: endpoint.name.clone(),
        init: CTerm::new(init, constraints.clone()),
        target: CTerm::new(target, constraints),
    })
}

fn dotk() -> Pattern {
    Pattern::app("dotk", vec![], vec![])
}

/// `\equals{SortBool{}, SortGeneratedTopCell{}}(\dv{SortBool{}}("true"), term)`: pyk's
/// `mlEqualsTrue`.
fn equals_true(term: Term) -> Pattern {
    let bool_sort = Sort::app0("SortBool");
    Pattern::Ml(
        kore::Ml::new(
            kore::Conn::Equals,
            vec![bool_sort.clone(), top_sort()],
            None,
            vec![
                Pattern::dv(bool_sort.clone(), "true"),
                term.into_sort(&bool_sort),
            ],
        )
        .expect("well-formed \\equals"),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names() {
        assert_eq!(var_name("ARG_0"), "VarARG'Unds'0");
        assert_eq!(
            var_name("?_ACCOUNTS_CELL"),
            "Var'QuesUnds'ACCOUNTS'Unds'CELL"
        );
        assert_eq!(var_name("_VMOUTPUT_CELL"), "Var'Unds'VMOUTPUT'Unds'CELL");
        assert_eq!(cell_var("prevBlockNonce"), "PREVBLOCKNONCE_CELL");
    }
}
