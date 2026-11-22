//! KORE text: parsing and printing.

mod lexer;
mod parser;
mod printer;

use std::fmt;

pub use parser::Parser;
pub use printer::{write_definition, write_module, write_pattern, write_sentence, write_sort};

use crate::syntax::{Definition, Pattern, Sort};

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ParseError {
    /// Byte offset in the input.
    pub offset: usize,
    /// 1-based line and column (in characters).
    pub line: usize,
    pub column: usize,
    pub message: String,
}

impl ParseError {
    pub(crate) fn at(src: &str, offset: usize, message: impl Into<String>) -> ParseError {
        let offset = offset.min(src.len());
        let before = src.get(..offset).unwrap_or(src);
        let line = before.matches('\n').count() + 1;
        let line_start = before.rfind('\n').map_or(0, |i| i + 1);
        let column = before[line_start..].chars().count() + 1;
        ParseError {
            offset,
            line,
            column,
            message: message.into(),
        }
    }
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}: {}", self.line, self.column, self.message)
    }
}

impl std::error::Error for ParseError {}

/// Parses a single pattern; the input must contain nothing else.
pub fn parse_pattern(src: &str) -> Result<Pattern, ParseError> {
    let mut parser = Parser::new(src)?;
    let pattern = parser.pattern()?;
    parser.expect_eof()?;
    Ok(pattern)
}

pub fn parse_sort(src: &str) -> Result<Sort, ParseError> {
    let mut parser = Parser::new(src)?;
    let sort = parser.sort()?;
    parser.expect_eof()?;
    Ok(sort)
}

pub fn parse_definition(src: &str) -> Result<Definition, ParseError> {
    let mut parser = Parser::new(src)?;
    let definition = parser.definition()?;
    parser.expect_eof()?;
    Ok(definition)
}

macro_rules! display_with {
    ($t:ty, $write:path) => {
        impl fmt::Display for $t {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                let mut out = String::new();
                $write(self, &mut out);
                f.write_str(&out)
            }
        }
    };
}

display_with!(crate::syntax::Sort, write_sort);
display_with!(crate::syntax::Pattern, write_pattern);
display_with!(crate::syntax::Sentence, write_sentence);
display_with!(crate::syntax::Module, write_module);
display_with!(crate::syntax::Definition, write_definition);

#[cfg(test)]
mod tests {
    use super::*;
    use crate::syntax::*;

    fn round_trip(src: &str) {
        let p = parse_pattern(src).unwrap_or_else(|e| panic!("{src}: {e}"));
        assert_eq!(p.to_string(), src);
    }

    #[test]
    fn patterns() {
        round_trip(r#"Lbl'Unds'Plus'Int'Unds'{}(X : SortInt{}, \dv{SortInt{}}("1"))"#);
        round_trip(r"inj{SortInt{}, SortKItem{}}(VarX : SortInt{})");
        round_trip(r"dotk{}()");
        round_trip(r"\top{SortGeneratedTopCell{}}()");
        round_trip(r"\and{S}(\not{S}(\bottom{S}()), \or{S}(), \or{S}(A : S, B : S, C : S))");
        round_trip(
            r"\exists{S}(X : SortInt{}, \equals{SortInt{}, S}(X : SortInt{}, Y : SortInt{}))",
        );
        round_trip(r"\mu{}(@X : S, \nu{}(@Y : S, @X : S))");
        round_trip(r"\ceil{SortInt{}, S}(X : SortInt{})");
        round_trip(r"\floor{SortInt{}, S}(X : SortInt{})");
        round_trip(r"\in{SortInt{}, S}(X : SortInt{}, Y : SortInt{})");
        round_trip(r"\rewrites{S}(\next{S}(A : S), B : S)");
        round_trip(r"\implies{S}(A : S, \iff{S}(A : S, B : S))");
        round_trip(r"\forall{S}(X : S, X : S)");
        round_trip(
            r"\left-assoc{}(Lbl'Unds'Map'Unds'{}(A : SortMap{}, B : SortMap{}, C : SortMap{}))",
        );
        round_trip(r"\right-assoc{}(\foo{}())");
        round_trip(r#""a\"b\\c\n\x00\u0100""#);
        round_trip(r#"\dv{SortBytes{}}("\x00\xff")"#);
    }

    #[test]
    fn whitespace_and_comments() {
        let p = parse_pattern("  f{} ( /* c */ a{}() , // line\n b{}()\n)  ").unwrap();
        assert_eq!(p.to_string(), "f{}(a{}(), b{}())");
    }

    #[test]
    fn errors() {
        for (src, line, column) in [
            ("f{}(", 1, 5),
            ("f{}(a{}() b{}())", 1, 11),
            (r"\not{S}()", 1, 1),
            (r"\exists{S}(@X : S, X : S)", 1, 1),
            (r"\mu{}(X : S, X : S)", 1, 1),
            (r"\top{S, S}()", 1, 1),
            ("f{}()\n  g{}()", 2, 3),
            ("\"abc", 1, 1),
            (r#""\q""#, 1, 1),
            ("f{}(#)", 1, 5),
        ] {
            let e = parse_pattern(src).expect_err(src);
            assert_eq!((e.line, e.column), (line, column), "{src}: {e}");
        }
    }

    #[test]
    fn definition() {
        let src = r#"[topCellInitializer{}(LblinitGeneratedTopCell{}())]

module M
    import N []
    sort SortInt{} [hook{}("INT.Int")]
    hooked-sort SortMap{} []
    symbol inj{From, To}(From) : To [sortInjection{}()]
    hooked-symbol Lbl'Unds'Plus'Int'Unds'{}(SortInt{}, SortInt{}) : SortInt{} [hook{}("INT.add")]
    alias rewrites{S}(S, S) : S where rewrites{S}(A : S, B : S) := \rewrites{S}(A : S, B : S) []
    axiom{R} \equals{SortInt{}, R}(X : SortInt{}, X : SortInt{}) [simplification{}("")]
    claim{} \top{SortInt{}}() []
endmodule [initialize{}()]

module N
endmodule []"#;
        let d = parse_definition(src).unwrap();
        assert_eq!(d.to_string(), src);
        assert_eq!(d.axioms().count(), 1);
        assert_eq!(d.symbol_decls().count(), 2);
    }

    #[test]
    fn deep() {
        // A cons list nested far deeper than any call stack allows.
        const N: usize = 1_000_000;
        let src = format!("{}nil{{}}(){}", "cons{}(a{}(), ".repeat(N), ")".repeat(N));
        let p = parse_pattern(&src).unwrap();
        assert_eq!(p.depth(), N + 1);
        assert_eq!(p.to_string(), src);
        // Rebuilt without change, and still not recursing.
        let p = p.map_bottom_up(|p| p);
        assert_eq!(p.depth(), N + 1);
        drop(p);
    }

    #[test]
    fn desugar_assoc() {
        let desugar = |s: &str| parse_pattern(s).unwrap().desugar_assoc().to_string();
        assert_eq!(
            desugar(r"\left-assoc{}(f{}(a{}(), b{}(), \left-assoc{}(g{}(c{}())), d{}()))"),
            "f{}(f{}(f{}(a{}(), b{}()), c{}()), d{}())"
        );
        assert_eq!(
            desugar(r"h{}(\right-assoc{}(f{}(a{}(), b{}(), c{}())))"),
            "h{}(f{}(a{}(), f{}(b{}(), c{}())))"
        );
        assert_eq!(desugar(r"\left-assoc{}(f{}())"), "f{}()");
    }

    #[test]
    fn sorts() {
        assert_eq!(
            parse_sort("SortList{SortInt{}, S}").unwrap().to_string(),
            "SortList{SortInt{}, S}"
        );
        assert_eq!(parse_sort("S").unwrap(), Sort::Var("S".into()));
    }
}
