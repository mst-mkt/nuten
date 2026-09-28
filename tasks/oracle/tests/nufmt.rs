//! Checks the cases of `test_data/nufmt` for what needs no nu, so that it runs without nu too.
//! A case must parse, since the oracle skips a file with syntax errors that nu rejects too.

use std::{fs, path::Path};

use nuten_format::{Config, invariants};

#[test]
fn nufmt_cases_parse_and_keep_invariants() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("test_data/nufmt");
    let paths: Vec<_> = fs::read_dir(&dir)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().is_some_and(|extension| extension == "nu"))
        .collect();
    assert!(!paths.is_empty(), "no .nu files in {}", dir.display());

    let config = Config::default();
    let problems: Vec<_> = paths
        .iter()
        .flat_map(|path| {
            let checked = invariants::check(&fs::read_to_string(path).unwrap(), &config);
            let name = path.file_name().unwrap().display().to_string();
            let violations = checked
                .violations
                .into_iter()
                .map(|violation| violation.to_string());
            let errors = checked
                .syntax_errors
                .into_iter()
                .map(|error| format!("syntax error: {}", error.kind));
            violations
                .chain(errors)
                .map(move |problem| format!("{name}: {problem}"))
        })
        .collect();
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}
