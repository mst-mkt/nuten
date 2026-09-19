//! Every UTF-8 `.nu` file of the corpus must keep the invariants of the parser
//! and, unless it has syntax errors, those of the formatter. Exactly the files
//! in `SYNTAX_ERRORS` have syntax errors. The corpus is the directory
//! `NUTEN_CORPUS` names, which the devShell sets; without it the test is
//! skipped.

#![expect(clippy::print_stderr, reason = "what is skipped is reported")]

use std::{
    collections::BTreeSet,
    env, fs, io, panic,
    path::{Path, PathBuf},
};

use nuten_format::{Config, invariants};

/// The files with syntax errors, relative to the corpus. Nu rejects each of
/// them too.
const SYNTAX_ERRORS: &[&str] = &[
    // The `{` on line 3 is not closed.
    "custom-menus/current_session_history_menu.nu",
];

#[test]
fn every_corpus_file_keeps_the_invariants() {
    let Some(dir) = env::var_os("NUTEN_CORPUS").map(PathBuf::from) else {
        eprintln!("skipping the corpus: NUTEN_CORPUS is not set");
        return;
    };
    let mut paths = Vec::new();
    nu_files_below(&dir, &mut paths);
    assert!(!paths.is_empty(), "no .nu files in {}", dir.display());

    let config = Config::default();
    let mut problems = Vec::new();
    let mut unseen: BTreeSet<_> = SYNTAX_ERRORS.iter().copied().collect();
    for path in paths {
        let name = path.strip_prefix(&dir).unwrap().display().to_string();
        let listed = unseen.remove(name.as_str());
        let src = match fs::read_to_string(&path) {
            Ok(src) => src,
            // Not UTF-8, which `parse` requires; the CLI does not format such a
            // file either.
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
        // A panic is a bug in nuten itself; catching it keeps the rest of the
        // corpus checked.
        match panic::catch_unwind(|| invariants::check(&src, &config)) {
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
            Err(payload) => {
                let message = payload
                    .downcast_ref::<String>()
                    .map(String::as_str)
                    .or_else(|| payload.downcast_ref::<&str>().copied())
                    .unwrap_or("a panic");
                problems.push(format!("{name}: panics: {message}"));
            },
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

/// Every `.nu` file below `dir`, in a fixed order, hidden directories aside.
fn nu_files_below(dir: &Path, paths: &mut Vec<PathBuf>) {
    let mut entries: Vec<_> = fs::read_dir(dir)
        .unwrap_or_else(|error| panic!("{}: {error}", dir.display()))
        .map(Result::unwrap)
        .collect();
    entries.sort_by_key(fs::DirEntry::path);
    for entry in entries {
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if entry.file_type().unwrap().is_dir() {
            if !name.starts_with('.') {
                nu_files_below(&entry.path(), paths);
            }
        } else if name.ends_with(".nu") {
            paths.push(entry.path());
        }
    }
}
