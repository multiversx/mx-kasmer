//! KORE JSON, the format of the K JSON-RPC servers (`kore-rpc`, `kore-rpc-booster`) and of pyk.
//!
//! [`Pattern`] and [`Sort`](crate::Sort) implement `Serialize` and `Deserialize`, so they can be
//! embedded in other serde types (JSON-RPC messages). The functions here read and write strings
//! with `serde_json`, without its nesting limit and with `serde_stacker`, which grows the stack
//! on the heap as needed: terms nest one level per list or map element.
//!
//! The output has the layout of pyk (`json.dumps(..., sort_keys=True)`): keys sorted, `", "` and
//! `": "` as separators, non-ASCII characters escaped. It can be compared with pyk's byte for byte.

mod term;

use std::io;

use serde::{Deserialize, Serialize};

use crate::syntax::Pattern;
pub use term::{Term, TermOwned};

pub type JsonError = serde_json::Error;

/// Reads a value with no limit on nesting: any type whose patterns use the implementations of
/// this crate, such as a JSON-RPC message.
pub fn from_str<'a, T: Deserialize<'a>>(src: &'a str) -> Result<T, JsonError> {
    let mut de = serde_json::Deserializer::from_str(src);
    de.disable_recursion_limit();
    let value = T::deserialize(serde_stacker::Deserializer::new(&mut de))?;
    de.end()?;
    Ok(value)
}

/// Reads a pattern in KORE JSON, without the wrapper.
pub fn parse_pattern(src: &str) -> Result<Pattern, JsonError> {
    from_str(src)
}

/// Reads a KORE JSON term: `{"format": "KORE", "version": 1, "term": pattern}`.
pub fn parse_term(src: &str) -> Result<Pattern, JsonError> {
    from_str::<term::TermOwned>(src).map(|t| t.0)
}

/// Writes JSON with pyk's separators and ASCII-only output.
struct PykFormatter;

impl serde_json::ser::Formatter for PykFormatter {
    fn begin_array_value<W: ?Sized + io::Write>(
        &mut self,
        w: &mut W,
        first: bool,
    ) -> io::Result<()> {
        if first { Ok(()) } else { w.write_all(b", ") }
    }

    fn begin_object_key<W: ?Sized + io::Write>(
        &mut self,
        w: &mut W,
        first: bool,
    ) -> io::Result<()> {
        if first { Ok(()) } else { w.write_all(b", ") }
    }

    fn begin_object_value<W: ?Sized + io::Write>(&mut self, w: &mut W) -> io::Result<()> {
        w.write_all(b": ")
    }

    fn write_string_fragment<W: ?Sized + io::Write>(
        &mut self,
        w: &mut W,
        fragment: &str,
    ) -> io::Result<()> {
        let mut plain = 0;
        for (i, c) in fragment.char_indices() {
            if c.is_ascii() {
                continue;
            }
            w.write_all(&fragment.as_bytes()[plain..i])?;
            for unit in c.encode_utf16(&mut [0; 2]) {
                write!(w, "\\u{unit:04x}")?;
            }
            plain = i + c.len_utf8();
        }
        w.write_all(&fragment.as_bytes()[plain..])
    }
}

/// Writes a value with pyk's layout and no limit on nesting.
pub fn to_writer<W: io::Write, T: Serialize + ?Sized>(w: W, value: &T) -> Result<(), JsonError> {
    let mut ser = serde_json::Serializer::with_formatter(w, PykFormatter);
    value.serialize(serde_stacker::Serializer::new(&mut ser))
}

fn to_string<T: Serialize + ?Sized>(value: &T) -> String {
    let mut out = Vec::new();
    to_writer(&mut out, value).expect("serializing a pattern into memory does not fail");
    String::from_utf8(out).expect("the output is ASCII")
}

/// The pattern as KORE JSON, without the wrapper.
pub fn pattern_to_string(pattern: &Pattern) -> String {
    to_string(pattern)
}

/// The pattern as a KORE JSON term, with the format wrapper.
pub fn term_to_string(pattern: &Pattern) -> String {
    to_string(&Term(pattern))
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
        assert!(
            parse_term(
                r#"{"format": "KORE", "version": 2, "term": {"tag": "String", "value": ""}}"#
            )
            .is_err()
        );
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
