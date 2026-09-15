//! Every `.nu` file of the corpus must parse, without a panic, into a tree
//! that restores it; syntax errors are not checked yet. The corpus is the
//! directory `NUTEN_CORPUS` names, which the devShell sets; without it the
//! test is skipped.

#![expect(clippy::print_stderr, reason = "what is skipped is reported")]

use std::{
    env, fs,
    path::{Path, PathBuf},
};

use nuten_parser::parse;

#[test]
fn every_corpus_file_parses_into_a_tree_that_restores_it() {
    let Some(dir) = env::var_os("NUTEN_CORPUS").map(PathBuf::from) else {
        eprintln!("skipping the corpus: NUTEN_CORPUS is not set");
        return;
    };
    let mut paths = Vec::new();
    nu_files_below(&dir, &mut paths);
    assert!(!paths.is_empty(), "no .nu files in {}", dir.display());
    for path in paths {
        // `parse` takes UTF-8 only; the CLI reports such a file instead of
        // formatting it.
        let src = match fs::read_to_string(&path) {
            Ok(src) => src,
            Err(error) => {
                eprintln!("skipping {}: {error}", path.display());
                continue;
            },
        };
        let parsed = parse(&src);
        assert_eq!(
            parsed.syntax().text(),
            src.as_str(),
            "{} does not roundtrip",
            path.display()
        );
    }
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
