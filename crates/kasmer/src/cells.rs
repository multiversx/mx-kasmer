//! Access to the cells of a configuration in KORE.
//!
//! A cell `<name>` is an application of the symbol `Lbl'-LT-'name'-GT-'` to its content. Cells are
//! found by walking down nested cells from the top; cells inside collections of cells (such as the
//! `<account>` cells in `<accounts>`) are not reached.

use kore::Pattern;
use kore::symbols::label_to_symbol;

use crate::Error;

/// The KORE symbol of the cell `<name>`.
pub fn cell_symbol(name: &str) -> String {
    label_to_symbol(&format!("<{name}>")).expect("cell names are ASCII")
}

fn is_cell(symbol: &str) -> bool {
    symbol.starts_with("Lbl'-LT-'") && symbol.ends_with("'-GT-'")
}

/// The content of the cell `<name>`.
pub fn cell<'a>(config: &'a Pattern, name: &str) -> Option<&'a Pattern> {
    let target = cell_symbol(name);
    let mut stack = vec![config];
    while let Some(p) = stack.pop() {
        let Pattern::App(app) = p else { continue };
        if !is_cell(&app.symbol) {
            continue;
        }
        if *app.symbol == *target {
            return app.args.first();
        }
        stack.extend(app.args.iter().rev());
    }
    None
}

pub fn cell_mut<'a>(config: &'a mut Pattern, name: &str) -> Option<&'a mut Pattern> {
    let target = cell_symbol(name);
    let mut stack = vec![config];
    while let Some(p) = stack.pop() {
        let Pattern::App(app) = p else { continue };
        if !is_cell(&app.symbol) {
            continue;
        }
        if *app.symbol == *target {
            return app.args.first_mut();
        }
        stack.extend(app.args.iter_mut().rev());
    }
    None
}

/// Replaces the content of the cell `<name>`.
pub fn set_cell(config: &mut Pattern, name: &str, value: Pattern) -> Result<(), Error> {
    let slot = cell_mut(config, name)
        .ok_or_else(|| Error::Project(format!("no cell <{name}> in the configuration")))?;
    *slot = value;
    Ok(())
}

/// The content of `<k>` is `.K`.
pub fn k_is_empty(config: &Pattern) -> bool {
    matches!(cell(config, "k"), Some(Pattern::App(app)) if &*app.symbol == "dotk" && app.args.is_empty())
}

/// The content of the cell in KORE text, or a note that there is no such cell.
pub fn cell_text(config: &Pattern, name: &str) -> String {
    cell(config, name).map_or_else(|| format!("(no <{name}>)"), |p| p.to_string())
}

/// The content of a cell of sort `String` (such as `<logging>`), decoded.
pub fn string_cell<'a>(config: &'a Pattern, name: &str) -> Option<&'a str> {
    match cell(config, name)? {
        Pattern::Dv { value, .. } => Some(value),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kore::text::parse_pattern;

    #[test]
    fn find_and_set() {
        let mut c = parse_pattern(
            r#"Lbl'-LT-'top'-GT-'{}(Lbl'-LT-'k'-GT-'{}(dotk{}()), Lbl'-LT-'x'-GT-'{}(Lbl'-LT-'logging'-GT-'{}(\dv{SortString{}}("hi"))))"#,
        )
        .unwrap();
        assert_eq!(cell_symbol("exit-code"), "Lbl'-LT-'exit-code'-GT-'");
        assert!(k_is_empty(&c));
        assert_eq!(string_cell(&c, "logging"), Some("hi"));
        set_cell(
            &mut c,
            "logging",
            Pattern::dv(kore::Sort::app0("SortString"), "x"),
        )
        .unwrap();
        assert_eq!(string_cell(&c, "logging"), Some("x"));
        assert!(cell(&c, "nope").is_none());
        assert!(set_cell(&mut c, "nope", Pattern::String(String::new())).is_err());
    }
}
