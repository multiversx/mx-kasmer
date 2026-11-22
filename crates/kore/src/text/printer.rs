//! Printer for KORE text, with the layout of pyk's printer (`Kore.text`), so that the output can
//! be compared with pyk's byte for byte.
//!
//! Patterns are printed with an explicit stack, so the nesting depth is limited by memory only.

use crate::quote::enquote_into;
use crate::syntax::*;

enum Work<'a> {
    Pattern(&'a Pattern),
    Str(&'static str),
}

pub fn write_sort(sort: &Sort, out: &mut String) {
    match sort {
        Sort::Var(name) => out.push_str(name),
        Sort::App(name, args) => {
            out.push_str(name);
            out.push('{');
            write_sep(args, out, write_sort);
            out.push('}');
        }
    }
}

fn write_sep<T>(items: &[T], out: &mut String, mut write: impl FnMut(&T, &mut String)) {
    for (i, item) in items.iter().enumerate() {
        if i > 0 {
            out.push_str(", ");
        }
        write(item, out);
    }
}

fn write_var(var: &Var, out: &mut String) {
    out.push_str(&var.name);
    out.push_str(" : ");
    write_sort(&var.sort, out);
}

fn write_string(s: &str, out: &mut String) {
    out.push('"');
    enquote_into(s, out);
    out.push('"');
}

/// Pushes `args` in reverse, separated by `", "`, followed by `close`, onto the work stack.
fn push_args<'a>(stack: &mut Vec<Work<'a>>, args: &'a [Pattern], close: &'static str) {
    stack.push(Work::Str(close));
    for (i, arg) in args.iter().enumerate().rev() {
        stack.push(Work::Pattern(arg));
        if i > 0 {
            stack.push(Work::Str(", "));
        }
    }
}

fn write_app_head(app: &App, out: &mut String) {
    out.push_str(&app.symbol);
    out.push('{');
    write_sep(&app.sorts, out, write_sort);
    out.push_str("}(");
}

pub fn write_pattern(pattern: &Pattern, out: &mut String) {
    let mut stack = vec![Work::Pattern(pattern)];
    while let Some(work) = stack.pop() {
        let p = match work {
            Work::Str(s) => {
                out.push_str(s);
                continue;
            }
            Work::Pattern(p) => p,
        };
        match p {
            Pattern::EVar(v) | Pattern::SVar(v) => write_var(v, out),
            Pattern::String(s) => write_string(s, out),
            Pattern::Dv { sort, value } => {
                out.push_str(r"\dv{");
                write_sort(sort, out);
                out.push_str("}(");
                write_string(value, out);
                out.push(')');
            }
            Pattern::App(app) => {
                write_app_head(app, out);
                push_args(&mut stack, &app.args, ")");
            }
            Pattern::Assoc(Assoc { dir, app }) => {
                out.push_str(match dir {
                    AssocDir::Left => r"\left-assoc{}(",
                    AssocDir::Right => r"\right-assoc{}(",
                });
                write_app_head(app, out);
                push_args(&mut stack, &app.args, "))");
            }
            Pattern::Ml(ml) => {
                out.push_str(ml.conn.symbol());
                out.push('{');
                write_sep(&ml.sorts, out, write_sort);
                out.push_str("}(");
                if let Some(var) = &ml.var {
                    write_var(var, out);
                    if !ml.args.is_empty() {
                        out.push_str(", ");
                    }
                }
                push_args(&mut stack, &ml.args, ")");
            }
        }
    }
}

fn write_attrs(attrs: &[Attr], out: &mut String) {
    out.push('[');
    for (i, attr) in attrs.iter().enumerate() {
        if i > 0 {
            out.push_str(", ");
        }
        write_pattern_app(attr, out);
    }
    out.push(']');
}

fn write_pattern_app(app: &App, out: &mut String) {
    write_app_head(app, out);
    for (i, arg) in app.args.iter().enumerate() {
        if i > 0 {
            out.push_str(", ");
        }
        write_pattern(arg, out);
    }
    out.push(')');
}

fn write_ids(ids: &[Id], out: &mut String) {
    write_sep(ids, out, |id, out| out.push_str(id));
}

fn write_signature(name: &Id, vars: &[Id], params: &[Sort], sort: &Sort, out: &mut String) {
    out.push_str(name);
    out.push('{');
    write_ids(vars, out);
    out.push_str("}(");
    write_sep(params, out, write_sort);
    out.push_str(") : ");
    write_sort(sort, out);
}

pub fn write_sentence(sentence: &Sentence, out: &mut String) {
    match sentence {
        Sentence::Import { module, attrs } => {
            out.push_str("import ");
            out.push_str(module);
            out.push(' ');
            write_attrs(attrs, out);
        }
        Sentence::Sort(d) => {
            out.push_str(if d.hooked { "hooked-sort " } else { "sort " });
            out.push_str(&d.name);
            out.push('{');
            write_ids(&d.vars, out);
            out.push_str("} ");
            write_attrs(&d.attrs, out);
        }
        Sentence::Symbol(d) => {
            out.push_str(if d.hooked { "hooked-symbol " } else { "symbol " });
            write_signature(&d.name, &d.vars, &d.params, &d.sort, out);
            out.push(' ');
            write_attrs(&d.attrs, out);
        }
        Sentence::Alias(d) => {
            out.push_str("alias ");
            write_signature(&d.name, &d.vars, &d.params, &d.sort, out);
            out.push_str(" where ");
            write_pattern_app(&d.left, out);
            out.push_str(" := ");
            write_pattern(&d.right, out);
            out.push(' ');
            write_attrs(&d.attrs, out);
        }
        Sentence::Axiom(a) | Sentence::Claim(a) => {
            out.push_str(if matches!(sentence, Sentence::Axiom(_)) {
                "axiom{"
            } else {
                "claim{"
            });
            write_ids(&a.vars, out);
            out.push_str("} ");
            write_pattern(&a.pattern, out);
            out.push(' ');
            write_attrs(&a.attrs, out);
        }
    }
}

pub fn write_module(module: &Module, out: &mut String) {
    out.push_str("module ");
    out.push_str(&module.name);
    for sentence in &module.sentences {
        out.push_str("\n    ");
        write_sentence(sentence, out);
    }
    out.push_str("\nendmodule ");
    write_attrs(&module.attrs, out);
}

pub fn write_definition(definition: &Definition, out: &mut String) {
    write_attrs(&definition.attrs, out);
    for module in &definition.modules {
        out.push_str("\n\n");
        write_module(module, out);
    }
}
