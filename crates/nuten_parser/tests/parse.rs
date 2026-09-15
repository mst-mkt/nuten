//! Snapshot tests of `parse`.
//! Each input in `test_data` has its dump in a `.snap` file next to it.
//! Inputs in `test_data/ok` parse without errors.
//! Inputs in `test_data/err` parse with at least one.

use std::{fmt::Write, fs, path::Path};

use nuten_parser::{Parsed, invariants, parse};
use rowan::WalkEvent;

#[test]
fn ok_inputs_parse_without_errors() {
    insta::glob!("../test_data", "ok/**/*.nu", |path| {
        let parsed = check_input(path);
        assert!(parsed.errors().is_empty(), "errors in {}", path.display());
    });
}

#[test]
fn err_inputs_parse_with_errors() {
    insta::glob!("../test_data", "err/**/*.nu", |path| {
        let parsed = check_input(path);
        assert!(
            !parsed.errors().is_empty(),
            "no errors in {}",
            path.display()
        );
    });
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
