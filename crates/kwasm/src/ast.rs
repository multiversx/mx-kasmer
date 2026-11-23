//! Constructors for the terms of the K Wasm semantics, the counterpart of pykwasm's
//! `kwasm_ast.py`. Terms are built by K label; [`Symbols`] finds the KORE symbols in the
//! definition and inserts the injections.

use kore::Sort;
use kore::symbols::{Symbols, Term};

use crate::Error;

pub(crate) type R<T> = Result<T, Error>;

/// Labels of the list constructors.
const DEFNS: &str = "___WASM-COMMON-SYNTAX_Defns_Defn_Defns";
const INSTRS: &str = "___WASM-COMMON-SYNTAX_Instrs_Instr_Instrs";
/// The empty `Defns` and `Instrs` are the same constant, of the common subsort `EmptyStmts`.
const EMPTY_STMTS: &str = ".List{\"listStmt\"}";

pub(crate) struct Ast<'a> {
    symbols: &'a Symbols,
}

impl<'a> Ast<'a> {
    pub fn new(symbols: &'a Symbols) -> Ast<'a> {
        Ast { symbols }
    }

    pub fn app(&self, label: &str, args: Vec<Term>) -> R<Term> {
        Ok(self.symbols.apply(label, args)?)
    }

    /// A constant: a symbol without arguments.
    pub fn c(&self, label: &str) -> R<Term> {
        self.app(label, vec![])
    }

    pub fn int(&self, value: impl ToString) -> Term {
        Term::dv(Sort::app0("SortInt"), value.to_string())
    }

    /// A Wasm name. As in pykwasm, the token is the name between double quotes, not escaped.
    pub fn wasm_string(&self, s: &str) -> Term {
        Term::dv(Sort::app0("SortWasmStringToken"), format!("\"{s}\""))
    }

    /// K `Bytes`: in KORE, a string with one character per byte.
    pub fn bytes(&self, bs: &[u8]) -> Term {
        Term::dv(
            Sort::app0("SortBytes"),
            bs.iter().map(|&b| char::from(b)).collect::<String>(),
        )
    }

    pub fn string(&self, s: &str) -> Term {
        Term::dv(Sort::app0("SortString"), s)
    }

    /// A Wasm float constant. pykwasm writes Python's `str` of the value (for `f32`, of the value
    /// widened to `f64`) as a K `Float` token.
    pub fn float(&self, value: f64) -> Term {
        Term::dv(Sort::app0("SortFloat"), python_float_repr(value))
    }

    /// A right-nested cons list ending in `nil`.
    pub fn list(&self, cons: &str, nil: &str, items: Vec<Term>) -> R<Term> {
        let mut tail = self.c(nil)?;
        for item in items.into_iter().rev() {
            tail = self.app(cons, vec![item, tail])?;
        }
        Ok(tail)
    }

    pub fn defns(&self, items: Vec<Term>) -> R<Term> {
        self.list(DEFNS, EMPTY_STMTS, items)
    }

    pub fn instrs(&self, items: Vec<Term>) -> R<Term> {
        self.list(INSTRS, EMPTY_STMTS, items)
    }

    pub fn val_types(&self, items: Vec<Term>) -> R<Term> {
        self.list("listValTypes", ".List{\"listValTypes\"}", items)
    }

    pub fn ints(&self, items: impl IntoIterator<Item = u32>) -> R<Term> {
        let items = items.into_iter().map(|i| self.int(i)).collect();
        self.list("listInt", ".List{\"listInt\"}", items)
    }

    pub fn vec_type(&self, val_types: Vec<Term>) -> R<Term> {
        let list = self.val_types(val_types)?;
        self.app("aVecType", vec![list])
    }

    pub fn empty_id(&self) -> R<Term> {
        self.c(".Identifier")
    }

    pub fn limits(&self, min: u64, max: Option<u64>) -> R<Term> {
        match max {
            None => self.app("limitsMin", vec![self.int(min)]),
            Some(max) => self.app("limitsMinMax", vec![self.int(min), self.int(max)]),
        }
    }

    pub fn instr_with_pos(&self, instr: Term, offset: u64, length: u64) -> R<Term> {
        self.app(
            "aInstrWithPos",
            vec![instr, self.int(offset), self.int(length)],
        )
    }

    /// `label(type, op)`, the shape of most numeric instructions: `aIBinOp(i32, intAdd)`.
    pub fn typed(&self, label: &str, ty: &str, op: &str) -> R<Term> {
        self.app(label, vec![self.c(ty)?, self.c(op)?])
    }
}

/// Python's `repr` of a float (also its `str`): the shortest digits that read back to the same
/// value, in positional notation when the decimal exponent is in [-4, 16), else in scientific
/// notation with a signed exponent of at least two digits.
pub(crate) fn python_float_repr(x: f64) -> String {
    if x.is_nan() {
        return "nan".into();
    }
    if x.is_infinite() {
        return if x < 0.0 { "-inf" } else { "inf" }.into();
    }
    // Rust's `{:e}` gives the shortest round-trip digits: "1.5e-5", "1e16", "0e0".
    let sci = format!("{:e}", x.abs());
    let (mantissa, exp) = sci.split_once('e').expect("`{:e}` has an exponent");
    let digits: String = mantissa.chars().filter(|c| *c != '.').collect();
    let exp: i32 = exp.parse().expect("integer exponent");
    // The value is 0.DIGITS * 10^point.
    let point = exp + 1;
    let sign = if x.is_sign_negative() { "-" } else { "" };
    let n = digits.len() as i32;
    let body = if (-3..=16).contains(&point) {
        if point <= 0 {
            format!("0.{}{digits}", "0".repeat((-point) as usize))
        } else if point >= n {
            format!("{digits}{}.0", "0".repeat((point - n) as usize))
        } else {
            format!(
                "{}.{}",
                &digits[..point as usize],
                &digits[point as usize..]
            )
        }
    } else {
        let mant = if n == 1 {
            digits.clone()
        } else {
            format!("{}.{}", &digits[..1], &digits[1..])
        };
        let e = point - 1;
        format!("{mant}e{}{:02}", if e < 0 { '-' } else { '+' }, e.abs())
    };
    format!("{sign}{body}")
}

#[cfg(test)]
mod tests {
    use super::python_float_repr;

    #[test]
    fn float_repr_like_python() {
        // Expected values from CPython's repr().
        for (x, s) in [
            (0.0, "0.0"),
            (-0.0, "-0.0"),
            (1.0, "1.0"),
            (1.5, "1.5"),
            (100.0, "100.0"),
            (0.0001, "0.0001"),
            (0.00001, "1e-05"),
            (1.5e-5, "1.5e-05"),
            (1e15, "1000000000000000.0"),
            (1e16, "1e+16"),
            (1.2345e16, "1.2345e+16"),
            (123456789.125, "123456789.125"),
            (0.1, "0.1"),
            (0.1f32 as f64, "0.10000000149011612"),
            (f64::MAX, "1.7976931348623157e+308"),
            (5e-324, "5e-324"),
            (f64::INFINITY, "inf"),
            (f64::NEG_INFINITY, "-inf"),
            (f64::NAN, "nan"),
        ] {
            assert_eq!(python_float_repr(x), s, "{x:?}");
        }
    }
}
