//! Checks [`kwasm::module_term`] against pykwasm and pyk: for every `.wasm` file under
//! `contracts/`, the term must be the one that `wasm2kast` followed by `kast_to_kore` gives,
//! byte for byte. Reference data from `scripts/kore-corpus.sh` in `.conformance/kore/` (or set
//! `KORE_CORPUS`); without it the test passes with a notice on stderr.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::Instant;

use kore::symbols::Symbols;

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn corpus() -> Option<PathBuf> {
    let dir = match std::env::var_os("KORE_CORPUS") {
        Some(d) => PathBuf::from(d),
        None => root().join(".conformance/kore"),
    };
    if dir.join("wasm/index.tsv").is_file() {
        Some(dir)
    } else {
        eprintln!(
            "no Wasm reference data in {}; run scripts/kore-corpus.sh",
            dir.display()
        );
        None
    }
}

fn read(path: &Path) -> String {
    fs::read_to_string(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

fn first_difference(actual: &str, expected: &str) -> String {
    let at = actual
        .bytes()
        .zip(expected.bytes())
        .take_while(|(a, b)| a == b)
        .count();
    let context = |s: &str| {
        let start = s.floor_char_boundary(at.saturating_sub(100));
        let end = s.ceil_char_boundary((at + 100).min(s.len()));
        s[start..end].to_owned()
    };
    format!(
        "differs at byte {at} (lengths {} and {})\n  actual:   {}\n  expected: {}",
        actual.len(),
        expected.len(),
        context(actual),
        context(expected)
    )
}

#[test]
fn contracts() {
    let Some(dir) = corpus() else { return };
    // The reference terms were made with the symbols of this definition.
    let definition = kore::text::parse_definition(&read(
        &dir.join("definitions/mx-semantics-llvm-kasmer.pyk.kore"),
    ))
    .expect("definition parses");
    let symbols = Symbols::new(&definition);

    let index = read(&dir.join("wasm/index.tsv"));
    let mut count = 0;
    for line in index.lines() {
        let (wasm_path, kore_file) = line.split_once('\t').expect("two columns");
        let wasm = fs::read(root().join(wasm_path)).unwrap_or_else(|e| panic!("{wasm_path}: {e}"));
        let expected = read(&dir.join("wasm").join(kore_file));

        let start = Instant::now();
        let term = kwasm::module_term(&wasm, Some(wasm_path), &symbols)
            .unwrap_or_else(|e| panic!("{wasm_path}: {e}"));
        let elapsed = start.elapsed();
        assert_eq!(term.sort, kore::Sort::app0("SortModuleDecl"));
        let actual = term.pattern.to_string();
        if actual != expected {
            panic!("{wasm_path}: {}", first_difference(&actual, &expected));
        }
        eprintln!(
            "{wasm_path}: {} bytes of Wasm, {} of KORE, {elapsed:.1?}",
            wasm.len(),
            actual.len()
        );
        count += 1;
    }
    assert!(count > 0, "empty index");
}

/// The test modules of upstream's binary parser (`tests/wat/`), compiled by `wat2wasm` when the
/// reference data was written, without a file name.
#[test]
fn wat_modules() {
    let Some(dir) = corpus() else { return };
    let definition = kore::text::parse_definition(&read(
        &dir.join("definitions/mx-semantics-llvm-kasmer.pyk.kore"),
    ))
    .expect("definition parses");
    let symbols = Symbols::new(&definition);

    let wat_dir = dir.join("wasm/wat");
    let mut names: Vec<_> = fs::read_dir(&wat_dir)
        .unwrap()
        .filter_map(|e| {
            e.unwrap()
                .file_name()
                .to_str()?
                .strip_suffix(".wasm")
                .map(str::to_owned)
        })
        .collect();
    names.sort();
    assert_eq!(
        names.len(),
        fs::read_dir(root().join("crates/kwasm/tests/wat"))
            .unwrap()
            .count()
            - 1,
        "one per .wat"
    );
    for name in names {
        let wasm = fs::read(wat_dir.join(format!("{name}.wasm"))).unwrap();
        let expected = read(&wat_dir.join(format!("{name}.kore")));
        let term =
            kwasm::module_term(&wasm, None, &symbols).unwrap_or_else(|e| panic!("{name}: {e}"));
        let actual = term.pattern.to_string();
        if actual != expected {
            panic!("{name}: {}", first_difference(&actual, &expected));
        }
        eprintln!(
            "{name}.wat: {} bytes of Wasm, {} of KORE",
            wasm.len(),
            actual.len()
        );
    }
}

/// What the semantics cannot represent is rejected with an error, not converted wrongly.
#[test]
fn unsupported() {
    let Some(dir) = corpus() else { return };
    let definition = kore::text::parse_definition(&read(
        &dir.join("definitions/mx-semantics-llvm-kasmer.pyk.kore"),
    ))
    .expect("definition parses");
    let symbols = Symbols::new(&definition);
    for (wat, expected) in [
        (
            "(module (func (result i32) f32.const 1 i32.reinterpret_f32))",
            "I32ReinterpretF32",
        ),
        (
            "(module (func (result i32) f32.const 1 i32.trunc_sat_f32_s))",
            "I32TruncSatF32S",
        ),
        ("(module (memory i64 1))", "memory64"),
        (
            "(module (memory 1) (memory 1) (func (result i32) i32.const 0 i32.load 1))",
            "multi-memory",
        ),
        ("(module (func (param v128)))", "v128"),
        (
            "(module (rec (type (func)) (type (func))))",
            "recursive types",
        ),
        (
            "(module (type $t (func (param i32) (result i32))) (func (param i32) (result i32) local.get 0 block (type $t) end))",
            "type index",
        ),
        (
            "(module (memory 1) (data \"x\") (func i32.const 0 i32.const 0 i32.const 1 memory.init 0))",
            "MemoryInit",
        ),
        ("(module (type (struct)))", "composite types"),
    ] {
        let wasm = wat::parse_str(wat).unwrap_or_else(|e| panic!("{wat}: {e}"));
        match kwasm::module_term(&wasm, None, &symbols) {
            Err(e @ kwasm::Error::Unsupported { .. }) => {
                assert!(e.to_string().contains(expected), "{wat}: {e}")
            }
            other => panic!("{wat}: expected rejection, got {:?}", other.map(|t| t.sort)),
        }
    }
    assert!(matches!(
        kwasm::module_term(b"\0asm\x02\0\0\0", None, &symbols),
        Err(kwasm::Error::Parse(_))
    ));
    // A definition without the Wasm semantics.
    let empty = Symbols::default();
    let wasm = wat::parse_str("(module (func))").unwrap();
    assert!(matches!(
        kwasm::module_term(&wasm, None, &empty),
        Err(kwasm::Error::Build(_))
    ));
}
