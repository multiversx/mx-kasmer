//! Writes KORE JSON in the layout of pyk (`json.dumps(..., sort_keys=True)`): keys sorted,
//! `", "` and `": "` as separators, non-ASCII characters escaped. The output can be compared with
//! pyk's byte for byte.

use std::fmt::Write;

use crate::syntax::*;

fn write_str(s: &str, out: &mut String) {
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\x08' => out.push_str("\\b"),
            '\x0c' => out.push_str("\\f"),
            ' '..='~' | '\x7f' => out.push(c),
            _ => {
                let mut units = [0u16; 2];
                for unit in c.encode_utf16(&mut units) {
                    let _ = write!(out, "\\u{unit:04x}");
                }
            }
        }
    }
    out.push('"');
}

fn key(k: &str, out: &mut String) {
    write_str(k, out);
    out.push_str(": ");
}

pub fn write_sort(sort: &Sort, out: &mut String) {
    match sort {
        Sort::Var(name) => {
            out.push('{');
            key("name", out);
            write_str(name, out);
            out.push_str(", \"tag\": \"SortVar\"}");
        }
        Sort::App(name, args) => {
            out.push('{');
            key("args", out);
            write_sorts(args, out);
            out.push_str(", ");
            key("name", out);
            write_str(name, out);
            out.push_str(", \"tag\": \"SortApp\"}");
        }
    }
}

fn write_sorts(sorts: &[Sort], out: &mut String) {
    out.push('[');
    for (i, s) in sorts.iter().enumerate() {
        if i > 0 {
            out.push_str(", ");
        }
        write_sort(s, out);
    }
    out.push(']');
}

fn tag(t: &str, out: &mut String) {
    key("tag", out);
    write_str(t, out);
}

enum Work<'a> {
    Pattern(&'a Pattern),
    Str(&'static str),
    /// Text that depends on the pattern, written after its arguments.
    Owned(String),
}

/// Pushes the arguments, separated by `", "` and preceded by `"key": `, so that they are written
/// next; then `after`.
fn push_args<'a>(stack: &mut Vec<Work<'a>>, args: &'a [Pattern], after: Work<'a>) {
    stack.push(after);
    for (i, arg) in args.iter().enumerate().rev() {
        stack.push(Work::Pattern(arg));
        if i > 0 {
            stack.push(Work::Str(", "));
        }
    }
}

fn sorted_fields(f: impl FnOnce(&mut String)) -> String {
    let mut s = String::new();
    f(&mut s);
    s
}

pub fn write_pattern(pattern: &Pattern, out: &mut String) {
    let mut stack = vec![Work::Pattern(pattern)];
    while let Some(work) = stack.pop() {
        let p = match work {
            Work::Str(s) => {
                out.push_str(s);
                continue;
            }
            Work::Owned(s) => {
                out.push_str(&s);
                continue;
            }
            Work::Pattern(p) => p,
        };
        out.push('{');
        match p {
            Pattern::EVar(v) | Pattern::SVar(v) => {
                key("name", out);
                write_str(&v.name, out);
                out.push_str(", ");
                key("sort", out);
                write_sort(&v.sort, out);
                out.push_str(", ");
                tag(if matches!(p, Pattern::EVar(_)) { "EVar" } else { "SVar" }, out);
                out.push('}');
            }
            Pattern::String(s) => {
                tag("String", out);
                out.push_str(", ");
                key("value", out);
                write_str(s, out);
                out.push('}');
            }
            Pattern::Dv { sort, value } => {
                key("sort", out);
                write_sort(sort, out);
                out.push_str(", ");
                tag("DV", out);
                out.push_str(", ");
                key("value", out);
                write_str(value, out);
                out.push('}');
            }
            Pattern::App(app) => {
                // args, name, sorts, tag
                key("args", out);
                out.push('[');
                let after = sorted_fields(|s| {
                    s.push_str("], ");
                    key("name", s);
                    write_str(&app.symbol, s);
                    s.push_str(", ");
                    key("sorts", s);
                    write_sorts(&app.sorts, s);
                    s.push_str(", \"tag\": \"App\"}");
                });
                push_args(&mut stack, &app.args, Work::Owned(after));
            }
            Pattern::Assoc(Assoc { dir, app }) => {
                // argss, sorts, symbol, tag
                key("argss", out);
                out.push('[');
                let after = sorted_fields(|s| {
                    s.push_str("], ");
                    key("sorts", s);
                    write_sorts(&app.sorts, s);
                    s.push_str(", ");
                    key("symbol", s);
                    write_str(&app.symbol, s);
                    s.push_str(", ");
                    tag(
                        if *dir == AssocDir::Left {
                            "LeftAssoc"
                        } else {
                            "RightAssoc"
                        },
                        s,
                    );
                    s.push('}');
                });
                push_args(&mut stack, &app.args, Work::Owned(after));
            }
            Pattern::Ml(ml) => write_ml(ml, out, &mut stack),
        }
    }
}

/// Writes the fields of a connective in sorted order; pattern fields go through the stack.
fn write_ml<'a>(ml: &'a Ml, out: &mut String, stack: &mut Vec<Work<'a>>) {
    let t = ml.conn.json_tag();
    let sort = |s: &mut String, k: &str, i: usize| {
        key(k, s);
        write_sort(&ml.sorts[i], s);
    };
    let var = |s: &mut String| {
        let v = ml.var.as_ref().expect("binder has a variable");
        s.push_str(", ");
        tag(t, s);
        s.push_str(", ");
        key("var", s);
        write_str(&v.name, s);
        s.push_str(", ");
        key("varSort", s);
        write_sort(&v.sort, s);
        s.push('}');
    };
    let args = &ml.args;
    match ml.conn {
        Conn::Top | Conn::Bottom => {
            sort(out, "sort", 0);
            out.push_str(", ");
            tag(t, out);
            out.push('}');
        }
        // arg, sort, tag
        Conn::Not => {
            key("arg", out);
            let after = sorted_fields(|s| {
                s.push_str(", ");
                sort(s, "sort", 0);
                s.push_str(", ");
                tag(t, s);
                s.push('}');
            });
            push_args(stack, args, Work::Owned(after));
        }
        // first, second, sort, tag
        Conn::Implies | Conn::Iff => {
            key("first", out);
            let after = sorted_fields(|s| {
                s.push_str(", ");
                sort(s, "sort", 0);
                s.push_str(", ");
                tag(t, s);
                s.push('}');
            });
            stack.push(Work::Owned(after));
            stack.push(Work::Pattern(&args[1]));
            stack.push(Work::Str(", \"second\": "));
            stack.push(Work::Pattern(&args[0]));
        }
        // patterns, sort, tag
        Conn::And | Conn::Or => {
            key("patterns", out);
            out.push('[');
            let after = sorted_fields(|s| {
                s.push_str("], ");
                sort(s, "sort", 0);
                s.push_str(", ");
                tag(t, s);
                s.push('}');
            });
            push_args(stack, args, Work::Owned(after));
        }
        // arg, sort, tag, var, varSort
        Conn::Exists | Conn::Forall => {
            key("arg", out);
            let after = sorted_fields(|s| {
                s.push_str(", ");
                sort(s, "sort", 0);
                var(s);
            });
            push_args(stack, args, Work::Owned(after));
        }
        // arg, tag, var, varSort
        Conn::Mu | Conn::Nu => {
            key("arg", out);
            push_args(stack, args, Work::Owned(sorted_fields(var)));
        }
        // arg, argSort, sort, tag
        Conn::Ceil | Conn::Floor => {
            key("arg", out);
            let after = sorted_fields(|s| {
                s.push_str(", ");
                sort(s, "argSort", 0);
                s.push_str(", ");
                sort(s, "sort", 1);
                s.push_str(", ");
                tag(t, s);
                s.push('}');
            });
            push_args(stack, args, Work::Owned(after));
        }
        // argSort, first, second, sort, tag
        Conn::Equals | Conn::In => {
            sort(out, "argSort", 0);
            out.push_str(", ");
            key("first", out);
            let after = sorted_fields(|s| {
                s.push_str(", ");
                sort(s, "sort", 1);
                s.push_str(", ");
                tag(t, s);
                s.push('}');
            });
            stack.push(Work::Owned(after));
            stack.push(Work::Pattern(&args[1]));
            stack.push(Work::Str(", \"second\": "));
            stack.push(Work::Pattern(&args[0]));
        }
        // dest, sort, tag
        Conn::Next => {
            key("dest", out);
            let after = sorted_fields(|s| {
                s.push_str(", ");
                sort(s, "sort", 0);
                s.push_str(", ");
                tag(t, s);
                s.push('}');
            });
            push_args(stack, args, Work::Owned(after));
        }
        // dest, sort, source, tag
        Conn::Rewrites => {
            key("dest", out);
            let mut middle = String::from(", ");
            sort(&mut middle, "sort", 0);
            middle.push_str(", \"source\": ");
            stack.push(Work::Owned(sorted_fields(|s| {
                s.push_str(", ");
                tag(t, s);
                s.push('}');
            })));
            stack.push(Work::Pattern(&args[0]));
            stack.push(Work::Owned(middle));
            stack.push(Work::Pattern(&args[1]));
        }
    }
}

/// The pattern with the wrapper of the KORE JSON format: `{"format": "KORE", "term": ..., "version": 1}`.
pub fn write_term(pattern: &Pattern, out: &mut String) {
    out.push_str("{\"format\": \"KORE\", \"term\": ");
    write_pattern(pattern, out);
    out.push_str(", \"version\": 1}");
}
