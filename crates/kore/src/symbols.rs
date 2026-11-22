//! Building well-sorted terms from K labels, as pyk's `kast_to_kore` does.
//!
//! A K production has a label (`aIConst`, `___WASM-COMMON-SYNTAX_Instrs_Instr_Instrs`); in
//! KORE its symbol is `Lbl` followed by the label, encoded ([`label_to_symbol`]). [`Symbols`]
//! reads the signatures of all symbols from a compiled definition. [`Symbols::apply`] applies a
//! symbol to arguments and wraps each argument whose sort is not the parameter sort in an
//! injection, `inj{Actual, Expected}(...)`, as pyk does. It does not check that `Actual` is a
//! subsort of `Expected`; the backends reject ill-sorted terms.

use std::collections::HashMap;
use std::fmt;

use crate::syntax::{Definition, Id, Pattern, Sort};

/// K's encoding of a character outside identifiers, inside a symbol name
/// (`StringUtil.asciiReadableEncodingKore` in the K frontend).
fn encode_char(c: char) -> Option<&'static str> {
    Some(match c {
        ' ' => "Spce",
        '!' => "Bang",
        '"' => "Quot",
        '#' => "Hash",
        '$' => "Dolr",
        '%' => "Perc",
        '&' => "And-",
        '\'' => "Apos",
        '(' => "LPar",
        ')' => "RPar",
        '*' => "Star",
        '+' => "Plus",
        ',' => "Comm",
        '.' => "Stop",
        '/' => "Slsh",
        ':' => "Coln",
        ';' => "SCln",
        '<' => "-LT-",
        '=' => "Eqls",
        '>' => "-GT-",
        '?' => "Ques",
        '@' => "-AT-",
        '[' => "LSqB",
        '\\' => "Bash",
        ']' => "RSqB",
        '^' => "Xor-",
        '_' => "Unds",
        '`' => "BQuo",
        '{' => "LBra",
        '|' => "Pipe",
        '}' => "RBra",
        '~' => "Tild",
        _ => return None,
    })
}

/// The KORE symbol of a K label: `Lbl` followed by the label, where each run of characters other
/// than letters, digits and `-` is replaced by the names of its characters between quotes:
/// `.List{"listStmt"}` becomes `Lbl'Stop'List'LBraQuot'listStmt'QuotRBra'`. `None` for
/// characters outside printable ASCII.
pub fn label_to_symbol(label: &str) -> Option<String> {
    let mut out = String::from("Lbl");
    let mut quoted = false;
    for c in label.chars() {
        if c.is_ascii_alphanumeric() || c == '-' {
            if quoted {
                out.push('\'');
                quoted = false;
            }
            out.push(c);
        } else {
            if !quoted {
                out.push('\'');
                quoted = true;
            }
            out.push_str(encode_char(c)?);
        }
    }
    if quoted {
        out.push('\'');
    }
    Some(out)
}

/// Parameter and result sorts of a symbol.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Signature {
    pub params: Vec<Sort>,
    pub sort: Sort,
}

/// The signatures of the symbols of a definition.
///
/// Symbols are found by label through [`label_to_symbol`]. The `symbol` attributes of the
/// definition are not used: K writes them only for some symbols, and for list units of generated
/// labels they hold a shortened label that several symbols share (`.List{"___WASM-COMMON-SYNTAX"}`).
#[derive(Clone, Debug, Default)]
pub struct Symbols {
    by_name: HashMap<Id, Signature>,
}

/// A pattern with its sort.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Term {
    pub pattern: Pattern,
    pub sort: Sort,
}

impl Term {
    /// A domain value: `\dv{sort}("value")`.
    pub fn dv(sort: Sort, value: impl Into<String>) -> Term {
        Term {
            pattern: Pattern::dv(sort.clone(), value),
            sort,
        }
    }

    /// The pattern as an argument of sort `expected`: injected if its sort is another one.
    pub fn into_sort(self, expected: &Sort) -> Pattern {
        if &self.sort == expected {
            self.pattern
        } else {
            inj(self.sort, expected.clone(), self.pattern)
        }
    }
}

/// `inj{from, to}(pattern)`
pub fn inj(from: Sort, to: Sort, pattern: Pattern) -> Pattern {
    Pattern::app("inj", vec![from, to], vec![pattern])
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum BuildError {
    UnknownLabel(String),
    Arity {
        symbol: Id,
        expected: usize,
        found: usize,
    },
}

impl fmt::Display for BuildError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BuildError::UnknownLabel(l) => write!(f, "no symbol for label {l:?} in the definition"),
            BuildError::Arity {
                symbol,
                expected,
                found,
            } => {
                write!(f, "{symbol} takes {expected} arguments, got {found}")
            }
        }
    }
}

impl std::error::Error for BuildError {}

impl Symbols {
    /// The non-parametric symbols of the definition. Parametric ones (`inj`, the K sequence
    /// builtins) need sort arguments and are built by hand.
    pub fn new(definition: &Definition) -> Symbols {
        let mut symbols = Symbols::default();
        for decl in definition.symbol_decls() {
            if !decl.vars.is_empty() {
                continue;
            }
            symbols.by_name.insert(
                decl.name.clone(),
                Signature {
                    params: decl.params.clone(),
                    sort: decl.sort.clone(),
                },
            );
        }
        symbols
    }

    pub fn signature(&self, symbol: &str) -> Option<&Signature> {
        self.by_name.get(symbol)
    }

    /// The symbol of a label, if the definition declares it. Surrounding spaces are ignored, as
    /// the K compiler ignores them.
    pub fn symbol(&self, label: &str) -> Option<(&Id, &Signature)> {
        self.by_name
            .get_key_value(label_to_symbol(label.trim())?.as_str())
    }

    /// Applies the symbol of `label` to the arguments, injecting each into its parameter sort.
    pub fn apply(&self, label: &str, args: Vec<Term>) -> Result<Term, BuildError> {
        let (name, sig) = self
            .symbol(label)
            .ok_or_else(|| BuildError::UnknownLabel(label.to_owned()))?;
        if args.len() != sig.params.len() {
            return Err(BuildError::Arity {
                symbol: name.clone(),
                expected: sig.params.len(),
                found: args.len(),
            });
        }
        let args = args
            .into_iter()
            .zip(&sig.params)
            .map(|(a, p)| a.into_sort(p))
            .collect();
        Ok(Term {
            pattern: Pattern::app(name.clone(), vec![], args),
            sort: sig.sort.clone(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::text::parse_definition;

    #[test]
    fn encoding() {
        assert_eq!(label_to_symbol("aIConst").unwrap(), "LblaIConst");
        assert_eq!(
            label_to_symbol(".List{\"listStmt\"}").unwrap(),
            "Lbl'Stop'List'LBraQuot'listStmt'QuotRBra'"
        );
        assert_eq!(
            label_to_symbol("___WASM-COMMON-SYNTAX_Instrs_Instr_Instrs").unwrap(),
            "Lbl'UndsUndsUnds'WASM-COMMON-SYNTAX'Unds'Instrs'Unds'Instr'Unds'Instrs"
        );
        assert_eq!(label_to_symbol("<k>").unwrap(), "Lbl'-LT-'k'-GT-'");
        assert_eq!(
            label_to_symbol("_^%Int__").unwrap(),
            "Lbl'UndsXor-Perc'Int'UndsUnds'"
        );
        assert_eq!(label_to_symbol("_&Int_").unwrap(), "Lbl'UndsAnd-'Int'Unds'");
        assert_eq!(label_to_symbol("é"), None);
    }

    #[test]
    fn apply_injects() {
        let d = parse_definition(
            r#"[]
module M
    symbol Lblf{}(SortB{}, SortB{}) : SortC{} []
    symbol Lbla{}() : SortA{} []
    symbol Lblb{}() : SortB{} []
    symbol inj{From, To}(From) : To []
endmodule []"#,
        )
        .unwrap();
        let s = Symbols::new(&d);
        let a = s.apply("a", vec![]).unwrap();
        let b = s.apply("b", vec![]).unwrap();
        let t = s.apply("f", vec![a, b]).unwrap();
        assert_eq!(t.sort, Sort::app0("SortC"));
        assert_eq!(
            t.pattern.to_string(),
            "Lblf{}(inj{SortA{}, SortB{}}(Lbla{}()), Lblb{}())"
        );
        assert_eq!(
            s.apply("f", vec![]),
            Err(BuildError::Arity {
                symbol: "Lblf".into(),
                expected: 2,
                found: 0
            })
        );
        assert!(matches!(
            s.apply("nope", vec![]),
            Err(BuildError::UnknownLabel(_))
        ));
        assert!(s.symbol("inj").is_none(), "parametric symbols are left out");
    }
}
