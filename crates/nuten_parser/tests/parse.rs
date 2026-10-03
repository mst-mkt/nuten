use std::{
    collections::BTreeSet,
    fmt::Write,
    fs,
    path::{Path, PathBuf},
};

use nuten_parser::{Parsed, invariants, parse};
use rowan::WalkEvent;

#[test]
fn ok_inputs_have_no_errors() {
    insta::glob!("../test_data", "ok/**/*.nu", |path| {
        let parsed = check_input(path);
        assert!(parsed.errors().is_empty(), "errors in {}", path.display());
    });
}

#[test]
fn err_inputs_have_errors() {
    insta::glob!("../test_data", "err/**/*.nu", |path| {
        let parsed = check_input(path);
        assert!(
            !parsed.errors().is_empty(),
            "no errors in {}",
            path.display()
        );
    });
}

#[test]
fn test_data_holds_only_inputs_and_snapshots() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("test_data");
    let names: BTreeSet<_> = files_below(&dir)
        .iter()
        .map(|path| path.strip_prefix(&dir).unwrap().to_path_buf())
        .collect();
    let strays: Vec<_> = names
        .iter()
        .filter(|name| !is_input(name) && !is_snapshot_of_input(name, &names))
        .map(|name| name.display().to_string())
        .collect();
    assert!(
        strays.is_empty(),
        "neither an input nor its snapshot:\n{}",
        strays.join("\n")
    );
}

fn is_input(name: &Path) -> bool {
    (name.starts_with("ok") || name.starts_with("err"))
        && name.extension().is_some_and(|extension| extension == "nu")
}

/// A pending `.snap.new` of an input is insta's to review, not a stray.
fn is_snapshot_of_input(name: &Path, names: &BTreeSet<PathBuf>) -> bool {
    name.to_str()
        .and_then(|name| {
            name.strip_suffix(".snap")
                .or_else(|| name.strip_suffix(".snap.new"))
        })
        .map(Path::new)
        .is_some_and(|input| is_input(input) && names.contains(input))
}

fn files_below(dir: &Path) -> Vec<PathBuf> {
    fs::read_dir(dir)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .flat_map(|path| {
            if path.is_dir() {
                files_below(&path)
            } else {
                vec![path]
            }
        })
        .collect()
}

fn check_input(path: &Path) -> Parsed {
    let src = fs::read_to_string(path).unwrap();
    let parsed = parse(&src);
    let mut settings = insta::Settings::clone_current();
    settings.set_snapshot_path(path.parent().unwrap());
    settings.remove_snapshot_suffix();
    settings.set_prepend_module_to_snapshot(false);
    settings.set_omit_expression(true);
    settings.bind(|| {
        let name = path.file_name().unwrap().to_str().unwrap();
        insta::assert_snapshot!(name, dump(&src, &parsed));
    });
    let violations = invariants::violations(&src, &parsed);
    assert!(violations.is_empty(), "{}: {violations:?}", path.display());
    parsed
}

/// Unlike rowan's `{:#?}`, this keeps the full text of long tokens.
fn dump(src: &str, parsed: &Parsed) -> String {
    let mut out = String::new();
    let mut depth = 0;
    for event in parsed.syntax().preorder_with_tokens() {
        match event {
            WalkEvent::Enter(element) => {
                let indent = "  ".repeat(depth);
                let (kind, range) = (element.kind(), element.text_range());
                match element.as_token() {
                    Some(token) => writeln!(out, "{indent}{kind:?}@{range:?} {:?}", token.text()),
                    None => writeln!(out, "{indent}{kind:?}@{range:?}"),
                }
                .unwrap();
                depth += 1;
            },
            WalkEvent::Leave(_) => depth -= 1,
        }
    }
    for error in parsed.errors() {
        let text = &src[error.range];
        writeln!(out, "error {:?} {text:?}: {:?}", error.range, error.kind).unwrap();
    }
    out
}
