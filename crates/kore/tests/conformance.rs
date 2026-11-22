//! Checks the parsers and printers against reference data written by pyk and by the LLVM
//! interpreter: `scripts/kore-corpus.sh` writes it into `.conformance/kore/` (or set
//! `KORE_CORPUS`). Without the data the tests pass with a notice on stderr.
//!
//! For every term in the data: the text and the JSON parse to the same pattern, and printing it
//! gives pyk's text and pyk's JSON byte for byte.

use std::fs;
use std::path::{Path, PathBuf};

use kore::{json, text};

fn corpus() -> Option<PathBuf> {
    let dir = match std::env::var_os("KORE_CORPUS") {
        Some(d) => PathBuf::from(d),
        None => Path::new(env!("CARGO_MANIFEST_DIR")).join("../../.conformance/kore"),
    };
    if dir.is_dir() {
        Some(dir)
    } else {
        eprintln!(
            "no KORE reference data in {}; run scripts/kore-corpus.sh",
            dir.display()
        );
        None
    }
}

/// Files in `dir` whose name ends in `suffix`, sorted, with the name minus the suffix.
fn files(dir: &Path, suffix: &str) -> Vec<(String, PathBuf)> {
    let mut out: Vec<_> = fs::read_dir(dir)
        .unwrap_or_else(|e| panic!("{}: {e}", dir.display()))
        .map(|e| e.unwrap().path())
        .filter_map(|p| {
            let name = p.file_name()?.to_str()?.strip_suffix(suffix)?.to_owned();
            Some((name, p))
        })
        .collect();
    out.sort();
    out
}

fn read(path: &Path) -> String {
    fs::read_to_string(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

/// Like `assert_eq!` on long strings: reports the first difference with some context.
fn assert_same(actual: &str, expected: &str, what: &str) {
    if actual == expected {
        return;
    }
    let at = actual.bytes().zip(expected.bytes()).take_while(|(a, b)| a == b).count();
    let context = |s: &str| {
        let start = s.floor_char_boundary(at.saturating_sub(80));
        let end = s.ceil_char_boundary((at + 80).min(s.len()));
        s[start..end].to_owned()
    };
    panic!(
        "{what}: differs at byte {at} (lengths {} and {})\n  actual:   {}\n  expected: {}",
        actual.len(),
        expected.len(),
        context(actual),
        context(expected)
    );
}

#[test]
fn definitions() {
    let Some(dir) = corpus() else { return };
    let dir = dir.join("definitions");
    let originals: Vec<_> = files(&dir, ".kore")
        .into_iter()
        .filter(|(n, _)| !n.ends_with(".pyk"))
        .collect();
    assert!(!originals.is_empty(), "no definitions in {}", dir.display());
    for (name, path) in originals {
        let definition = text::parse_definition(&read(&path)).unwrap_or_else(|e| panic!("{name}: {e}"));
        let expected = read(&dir.join(format!("{name}.pyk.kore")));
        assert_same(&definition.to_string(), &expected, &name);
        eprintln!(
            "{name}: {} modules, {} axioms",
            definition.modules.len(),
            definition.axioms().count()
        );
    }
}

#[test]
fn axioms() {
    let Some(dir) = corpus() else { return };
    let dir = dir.join("axioms");
    let sets = files(&dir, ".kore");
    assert!(!sets.is_empty(), "no axioms in {}", dir.display());
    for (name, path) in sets {
        let texts = read(&path);
        let jsons = read(&dir.join(format!("{name}.jsonl")));
        let mut count = 0;
        for (i, (t, j)) in texts.lines().zip(jsons.lines()).enumerate() {
            let what = format!("{name}, axiom {i}");
            let from_text = text::parse_pattern(t).unwrap_or_else(|e| panic!("{what}: {e}"));
            let from_json = json::parse_pattern(j).unwrap_or_else(|e| panic!("{what}: {e}"));
            assert!(from_text == from_json, "{what}: text and JSON differ");
            assert_same(&from_text.to_string(), t, &what);
            assert_same(&json::pattern_to_string(&from_text), j, &what);
            count += 1;
        }
        assert_eq!(count, texts.lines().count());
        assert_eq!(count, jsons.lines().count());
        eprintln!("{name}: {count} axioms");
    }
}

#[test]
fn configurations() {
    let Some(dir) = corpus() else { return };
    let dir = dir.join("configs");
    let configs: Vec<_> = files(&dir, ".kore")
        .into_iter()
        .filter(|(n, _)| !n.ends_with(".interpreter"))
        .collect();
    assert!(!configs.is_empty(), "no configurations in {}", dir.display());
    for (name, path) in configs {
        let t = read(&path);
        let j = read(&dir.join(format!("{name}.json")));
        let from_text = text::parse_pattern(&t).unwrap_or_else(|e| panic!("{name}: {e}"));
        let from_json = json::parse_term(&j).unwrap_or_else(|e| panic!("{name}: {e}"));
        assert!(from_text == from_json, "{name}: text and JSON differ");
        assert_same(&from_text.to_string(), &t, &name);
        assert_same(&json::term_to_string(&from_text), &j, &name);

        let raw = dir.join(format!("{name}.interpreter.kore"));
        if raw.exists() {
            let from_interpreter = text::parse_pattern(&read(&raw)).unwrap_or_else(|e| panic!("{name}: {e}"));
            assert!(
                from_interpreter == from_text,
                "{name}: interpreter output differs from pyk's reading of it"
            );
        }
        eprintln!("{name}: depth {}", from_text.depth());
    }
}

/// The encoding of labels into symbol names agrees with the K compiler on every symbol that
/// records its label in a `symbol` attribute, except list units of generated labels, whose
/// attribute holds a shortened label.
#[test]
fn label_encoding() {
    let Some(dir) = corpus() else { return };
    let dir = dir.join("definitions");
    for (name, path) in files(&dir, ".pyk.kore") {
        let definition = text::parse_definition(&read(&path)).unwrap();
        let mut count = 0;
        for decl in definition.symbol_decls() {
            let label = decl
                .attrs
                .iter()
                .find(|a| &*a.symbol == "symbol'Kywd'")
                .and_then(|a| match a.args.first() {
                    Some(kore::Pattern::String(s)) => Some(s.as_str()),
                    _ => None,
                });
            let Some(label) = label else { continue };
            if label.starts_with(".List{\"___") {
                continue;
            }
            // A label written with surrounding spaces in the K source (" memImportDesc") keeps
            // them in the attribute; the compiler trims them for the symbol name.
            let encoded = kore::symbols::label_to_symbol(label.trim());
            assert_eq!(encoded.as_deref(), Some(&*decl.name), "{name}: {label}");
            count += 1;
        }
        assert!(count > 100, "{name}: only {count} labels");
        eprintln!("{name}: {count} labels encode to their symbols");
    }
}
