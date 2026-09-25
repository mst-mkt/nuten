//! Runs the binary on files in a temporary directory.
//! Without nu on PATH the tests that need it are skipped.

#![expect(clippy::print_stderr, reason = "what is skipped is reported")]

use std::{
    env, fs,
    path::PathBuf,
    process::{Command, Output},
    sync::atomic::{AtomicUsize, Ordering},
    thread,
};

/// A fresh directory for each test, removed when dropped.
struct TempDir(PathBuf);

impl TempDir {
    fn new() -> TempDir {
        static COUNT: AtomicUsize = AtomicUsize::new(0);
        let count = COUNT.fetch_add(1, Ordering::Relaxed);
        let dir = env::temp_dir().join(format!("nuten-oracle-{}-{count}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        TempDir(dir)
    }

    fn file(&self, name: &str, contents: &str) -> PathBuf {
        let path = self.0.join(name);
        fs::write(&path, contents).unwrap();
        path
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).ok();
    }
}

fn compare(args: &[&PathBuf]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_nuten-oracle"))
        .arg("compare")
        .args(args)
        .output()
        .unwrap()
}

/// The crane build that runs the tests has no nu.
fn nu_missing() -> bool {
    let missing = Command::new("nu").arg("--version").output().is_err();
    if missing {
        let test = thread::current().name().unwrap_or_default().to_string();
        eprintln!("skipping {test}: nu is not on PATH");
    }
    missing
}

#[test]
fn compare_passes_files_that_differ_only_in_layout() {
    if nu_missing() {
        return;
    }
    let dir = TempDir::new();
    let before = dir.file("before.nu", "ls | length\n");
    let after = dir.file("after.nu", "ls  |  length\n");

    let output = compare(&[&before, &after]);

    assert_eq!(output.status.code(), Some(0), "{output:?}");
}

#[test]
fn compare_fails_on_files_nu_parses_differently() {
    if nu_missing() {
        return;
    }
    let dir = TempDir::new();
    let before = dir.file("before.nu", "ls | length\n");
    let after = dir.file("after.nu", "ls; length\n");

    let output = compare(&[&before, &after]);

    assert_eq!(output.status.code(), Some(1), "{output:?}");
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("the AST differs"), "{stdout}");
}

#[test]
fn compare_fails_on_file_it_cannot_read() {
    if nu_missing() {
        return;
    }
    let dir = TempDir::new();
    let before = dir.file("before.nu", "ls | length\n");
    let after = dir.0.join("missing.nu");

    let output = compare(&[&before, &after]);

    assert_eq!(output.status.code(), Some(1), "{output:?}");
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("missing.nu"), "{stdout}");
}

#[test]
fn compare_takes_exactly_two_files() {
    let dir = TempDir::new();
    let file = dir.file("a.nu", "ls\n");

    let outputs = [compare(&[&file]), compare(&[&file, &file, &file])];

    for output in outputs {
        assert_eq!(output.status.code(), Some(2), "{output:?}");
    }
}
