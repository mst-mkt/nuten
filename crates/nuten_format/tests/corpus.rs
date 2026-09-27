//! Every UTF-8 `.nu` and `.nuon` file of the corpus must keep the invariants of the parser.
//! Unless it has syntax errors, it must keep those of the formatter too.
//! Exactly the files in `SYNTAX_ERRORS` have syntax errors.
//! The corpus is the directory `NUTEN_CORPUS` names, which the devShell sets.
//! Without it the test is skipped.

#![expect(clippy::print_stderr, reason = "what is skipped is reported")]

mod support;

use std::{
    borrow::Cow,
    collections::BTreeSet,
    env, fs, io,
    path::{Path, PathBuf},
};

use nuten_format::{Config, invariants};

use crate::support::{caught, files_below};

/// The files with syntax errors, relative to the corpus.
/// Nu rejects each of them too.
const SYNTAX_ERRORS: &[&str] = &[
    // The `{` on line 3 is not closed.
    "nu_scripts/custom-menus/current_session_history_menu.nu",
    // The `+` on line 75 has no right operand.
    "tree-sitter-nu/expr/binary-expr.nu",
    // The `-1` on line 166 is a negative index in a cell path.
    "tree-sitter-nu/expr/cell_path.nu",
];

#[test]
fn every_file_keeps_invariants_with_syntax_errors_as_listed() {
    let Some(dir) = env::var_os("NUTEN_CORPUS").map(PathBuf::from) else {
        eprintln!("skipping the corpus: NUTEN_CORPUS is not set");
        return;
    };
    // Skipping hidden directories spares walking `.git` when the corpus is a clone.
    // None of its scripts lives in one.
    let paths: Vec<_> = files_below(&dir, &|subdir| !name_of(subdir).starts_with('.'))
        .into_iter()
        .filter(|path| {
            path.extension()
                .is_some_and(|extension| extension == "nu" || extension == "nuon")
        })
        .collect();
    for extension in ["nu", "nuon"] {
        assert!(
            paths
                .iter()
                .any(|path| path.extension().is_some_and(|found| found == extension)),
            "no .{extension} files in {}",
            dir.display()
        );
    }

    let config = Config::default();
    let mut problems = Vec::new();
    let mut unseen: BTreeSet<_> = SYNTAX_ERRORS.iter().copied().collect();
    for path in paths {
        let name = path.strip_prefix(&dir).unwrap().display().to_string();
        let listed = unseen.remove(name.as_str());
        let src = match fs::read_to_string(&path) {
            Ok(src) => src,
            // Not UTF-8, which `parse` requires; the CLI does not format such a file either.
            Err(error) if error.kind() == io::ErrorKind::InvalidData => {
                if listed {
                    problems.push(format!("{name}: in SYNTAX_ERRORS, but not UTF-8"));
                }
                eprintln!("skipping {name}: {error}");
                continue;
            },
            Err(error) => {
                problems.push(format!("{name}: {error}"));
                continue;
            },
        };
        match caught(|| invariants::check(&src, &config)) {
            Ok(checked) => {
                let mislisting = match (listed, checked.syntax_errors.first()) {
                    (true, None) => Some(format!("{name}: in SYNTAX_ERRORS, but no syntax errors")),
                    (false, Some(error)) => Some(format!(
                        "{name}: {} at {:?}, but not in SYNTAX_ERRORS",
                        error.kind, error.range
                    )),
                    (true, Some(_)) | (false, None) => None,
                };
                problems.extend(mislisting);
                problems.extend(
                    checked
                        .violations
                        .iter()
                        .map(|violation| format!("{name}: {violation}")),
                );
            },
            Err(message) => problems.push(format!("{name}: panics: {message}")),
        }
    }
    problems.extend(
        unseen
            .iter()
            .map(|name| format!("{name}: in SYNTAX_ERRORS, but not in the corpus")),
    );
    assert!(
        problems.is_empty(),
        "problems in the corpus:\n{}",
        problems.join("\n")
    );
}

fn name_of(path: &Path) -> Cow<'_, str> {
    path.file_name().unwrap_or_default().to_string_lossy()
}
