//! KORE JSON, the format of the K JSON-RPC servers (`kore-rpc`, `kore-rpc-booster`) and of pyk.

mod reader;
mod writer;

pub use reader::{JsonError, parse_pattern, parse_term};
pub use writer::{write_pattern, write_sort, write_term};

use crate::syntax::Pattern;

/// The pattern as KORE JSON, without the wrapper.
pub fn pattern_to_string(pattern: &Pattern) -> String {
    let mut out = String::new();
    write_pattern(pattern, &mut out);
    out
}

/// The pattern as a KORE JSON term, with the format wrapper.
pub fn term_to_string(pattern: &Pattern) -> String {
    let mut out = String::new();
    write_term(pattern, &mut out);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::text;

    /// Text → JSON → pattern gives the same pattern, for every kind of pattern.
    #[test]
    fn round_trip() {
        for src in [
            r#"Lbl'Unds'Plus'Int'Unds'{}(X : SortInt{}, \dv{SortInt{}}("1"))"#,
            r"inj{SortInt{}, SortKItem{}}(VarX : SortInt{})",
            r"\and{S}(\not{S}(\bottom{S}()), \or{S}(), \or{S}(A : S, B : S, C : S))",
            r"\exists{S}(X : SortInt{}, \equals{SortInt{}, S}(X : SortInt{}, Y : SortInt{}))",
            r"\mu{}(@X : S, \nu{}(@Y : S, @X : S))",
            r"\ceil{SortInt{}, S}(X : SortInt{})",
            r"\floor{SortInt{}, S}(X : SortInt{})",
            r"\in{SortInt{}, S}(X : SortInt{}, Y : SortInt{})",
            r"\rewrites{S}(\next{S}(A : S), B : S)",
            r"\implies{S}(A : S, \iff{S}(A : S, B : S))",
            r"\forall{S}(X : S, \top{S}())",
            r"\left-assoc{}(Lbl'Unds'Map'Unds'{}(A : SortMap{}, B : SortMap{}, C : SortMap{}))",
            r"\right-assoc{}(\foo{}())",
            r#""a\"b\\c\n\x00\x7f\xe9\u0100\U0001f600""#,
        ] {
            let p = text::parse_pattern(src).unwrap();
            let json = pattern_to_string(&p);
            assert_eq!(parse_pattern(&json).unwrap(), p, "{json}");
            assert_eq!(parse_term(&term_to_string(&p)).unwrap(), p);
        }
    }

    #[test]
    fn layout_like_pyk() {
        let p = text::parse_pattern(r#"f{}(\dv{SortInt{}}("1"), "\xe9")"#).unwrap();
        assert_eq!(
            pattern_to_string(&p),
            r#"{"args": [{"sort": {"args": [], "name": "SortInt", "tag": "SortApp"}, "tag": "DV", "value": "1"}, {"tag": "String", "value": "\u00e9"}], "name": "f", "sorts": [], "tag": "App"}"#
        );
    }

    #[test]
    fn field_order_does_not_matter() {
        let json = r#"{"tag": "App", "name": "f", "sorts": [], "args": [{"value": "x", "tag": "String"}]}"#;
        assert_eq!(parse_pattern(json).unwrap().to_string(), r#"f{}("x")"#);
    }

    #[test]
    fn errors() {
        for json in [
            r#"{"tag": "App", "name": "f", "sorts": []}"#,
            r#"{"tag": "Not", "sort": {"tag": "SortVar", "name": "S"}, "arg": "x"}"#,
            r#"{"tag": "Nope"}"#,
            r#"{"tag": "SVar", "name": "X", "sort": {"tag": "SortVar", "name": "S"}}"#,
            r#"{"tag": "String", "value": "\ud800"}"#,
            r#"{"tag": "String", "value": "x"} x"#,
            r#"[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]"#,
        ] {
            assert!(parse_pattern(json).is_err(), "{json}");
        }
        assert!(parse_term(r#"{"format": "KORE", "version": 2, "term": {"tag": "String", "value": ""}}"#).is_err());
    }

    #[test]
    fn deep() {
        const N: usize = 1_000_000;
        let src = format!("{}nil{{}}(){}", "cons{}(a{}(), ".repeat(N), ")".repeat(N));
        let p = text::parse_pattern(&src).unwrap();
        let json = pattern_to_string(&p);
        let q = parse_pattern(&json).unwrap();
        assert_eq!(q.depth(), N + 1);
        assert_eq!(q.to_string(), src);
    }
}
