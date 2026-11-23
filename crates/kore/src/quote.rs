//! String literals of KORE text, as pyk quotes them (`pyk.dequote`).
//!
//! Escapes: `\"`, `\\`, `\n`, `\t`, `\r`, `\f`, and code points `\xHH`, `\uHHHH`,
//! `\UHHHHHHHH`. When quoting, printable ASCII is kept, the six characters above use their short
//! escape, everything else up to 0xFF uses `\x`, the rest `\u` or `\U` (lower-case hex).

use std::fmt::Write;

/// Appends `s` quoted, without the surrounding double quotes.
pub fn enquote_into(s: &str, out: &mut String) {
    for c in s.chars() {
        match c {
            '\t' => out.push_str(r"\t"),
            '\n' => out.push_str(r"\n"),
            '\x0c' => out.push_str(r"\f"),
            '\r' => out.push_str(r"\r"),
            '"' => out.push_str(r#"\""#),
            '\\' => out.push_str(r"\\"),
            ' '..='~' => out.push(c),
            _ => {
                let code = c as u32;
                // Writing to a String does not fail.
                let _ = if code <= 0xff {
                    write!(out, r"\x{code:02x}")
                } else if code <= 0xffff {
                    write!(out, r"\u{code:04x}")
                } else {
                    write!(out, r"\U{code:08x}")
                };
            }
        }
    }
}

pub fn enquote(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    enquote_into(s, &mut out);
    out
}

/// Decodes the inside of a string literal (without the double quotes).
pub fn dequote(s: &str) -> Result<String, String> {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        let escaped = chars.next().ok_or("incomplete escape sequence")?;
        let digits = match escaped {
            '"' | '\\' => {
                out.push(escaped);
                continue;
            }
            'n' => {
                out.push('\n');
                continue;
            }
            't' => {
                out.push('\t');
                continue;
            }
            'r' => {
                out.push('\r');
                continue;
            }
            'f' => {
                out.push('\x0c');
                continue;
            }
            'x' => 2,
            'u' => 4,
            'U' => 8,
            other => return Err(format!(r"unexpected escape sequence: \{other}")),
        };
        let mut code: u32 = 0;
        for _ in 0..digits {
            let d = chars.next().ok_or("incomplete code point")?;
            let v = d
                .to_digit(16)
                .ok_or_else(|| format!("expected hex digit, got: {d}"))?;
            code = code.checked_mul(16).ok_or("code point out of range")? + v;
        }
        out.push(
            char::from_u32(code).ok_or_else(|| format!("not a Unicode scalar value: {code:#x}"))?,
        );
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip() {
        for s in [
            "",
            "abc",
            "a\"b\\c",
            "\t\n\r\x0c",
            "\x00\x7f\u{ff}",
            "\u{100}\u{ffff}\u{10000}",
        ] {
            assert_eq!(dequote(&enquote(s)).unwrap(), s);
        }
    }

    #[test]
    fn like_pyk() {
        assert_eq!(
            enquote("\x00\x7f\u{e9}\u{100}\u{1f600}"),
            r"\x00\x7f\xe9\u0100\U0001f600"
        );
        assert_eq!(enquote("a\"b\\"), r#"a\"b\\"#);
    }

    #[test]
    fn errors() {
        assert!(dequote(r"\q").is_err());
        assert!(dequote(r"\x4").is_err());
        assert!(dequote(r"\ud800").is_err());
        assert!(dequote("\\").is_err());
    }
}
