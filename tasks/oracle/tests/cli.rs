//! Runs the binary on files in a temporary directory.
//! Without nu on PATH the tests that need it are skipped.

#![expect(clippy::print_stderr, reason = "what is skipped is reported")]

use std::{
    env,
    ffi::OsStr,
    fs,
    path::{Path, PathBuf},
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
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, contents).unwrap();
        path
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).ok();
    }
}

fn oracle(subcommand: &str, paths: &[&PathBuf]) -> Output {
    run(&[OsStr::new(subcommand)], paths)
}

fn oracle_with_config(config: &Path, paths: &[&PathBuf]) -> Output {
    run(
        &[
            OsStr::new("format"),
            OsStr::new("--config"),
            config.as_os_str(),
        ],
        paths,
    )
}

fn run(args: &[&OsStr], paths: &[&PathBuf]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_nuten-oracle"))
        .args(args)
        .args(paths)
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

    let output = oracle("compare", &[&before, &after]);

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

    let output = oracle("compare", &[&before, &after]);

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

    let output = oracle("compare", &[&before, &after]);

    assert_eq!(output.status.code(), Some(1), "{output:?}");
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("missing.nu"), "{stdout}");
}

#[test]
fn compare_takes_exactly_two_files() {
    let dir = TempDir::new();
    let file = dir.file("a.nu", "ls\n");

    let outputs = [
        oracle("compare", &[&file]),
        oracle("compare", &[&file, &file, &file]),
    ];

    for output in outputs {
        assert_eq!(output.status.code(), Some(2), "{output:?}");
    }
}

#[test]
fn format_passes_file_whose_ast_nuten_keeps() {
    if nu_missing() {
        return;
    }
    let dir = TempDir::new();
    let file = dir.file("a.nu", "ls|length\n");

    let output = oracle("format", &[&file]);

    assert_eq!(output.status.code(), Some(0), "{output:?}");
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(
        stderr.contains("files: 1, failed: 0, skipped: 0"),
        "{stderr}"
    );
}

#[test]
fn format_keeps_io_types_seen_by_calls_above_defs() {
    if nu_missing() {
        return;
    }
    let dir = TempDir::new();
    let file = dir.file(
        "a.nu",
        concat!(
            "def takes-str [x: string] { $x }\n",
            "takes-str (touching)\n",
            "takes-str (long-touching)\n",
            "takes-str (apart)\n",
            "def touching []: [nothing -> int] { 1 }\n",
            "def long-touching [--some-long-flag-name: string, --another-long-flag-name: int, ",
            "--third: int]: [nothing -> int] { 1 }\n",
            "def apart []: [nothing -> string ] { \"a\" }\n",
        ),
    );

    let output = oracle("format", &[&file]);

    assert_eq!(output.status.code(), Some(0), "{output:?}");
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(
        stderr.contains("files: 1, failed: 0, skipped: 0"),
        "{stderr}"
    );
}

#[test]
fn format_skips_file_with_syntax_errors() {
    if nu_missing() {
        return;
    }
    let dir = TempDir::new();
    let file = dir.file("a.nu", "ls | (\n");

    let output = oracle("format", &[&file]);

    assert_eq!(output.status.code(), Some(0), "{output:?}");
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("syntax errors"), "{stderr}");
    assert!(stderr.contains("skipped: 1"), "{stderr}");
}

#[test]
fn format_takes_nu_files_below_directory_but_not_in_hidden_ones() {
    if nu_missing() {
        return;
    }
    let dir = TempDir::new();
    dir.file("a.nu", "ls\n");
    dir.file("sub/b.nu", "ls\n");
    dir.file("sub/c.txt", "ls\n");
    dir.file(".hidden/d.nu", "ls\n");

    let output = oracle("format", &[&dir.0]);

    assert_eq!(output.status.code(), Some(0), "{output:?}");
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("files: 2,"), "{stderr}");
}

#[test]
fn format_exits_with_2_when_paths_hold_no_nu_file() {
    if nu_missing() {
        return;
    }
    let dir = TempDir::new();
    dir.file("a.txt", "ls\n");

    let output = oracle("format", &[&dir.0]);

    assert_eq!(output.status.code(), Some(2), "{output:?}");
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("no .nu or .nuon files"), "{stderr}");
}

#[test]
fn format_checks_file_given_twice_once() {
    if nu_missing() {
        return;
    }
    let dir = TempDir::new();
    let file = dir.file("a.nu", "ls\n");

    let output = oracle("format", &[&file, &file]);

    assert_eq!(output.status.code(), Some(0), "{output:?}");
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("files: 1,"), "{stderr}");
}

#[test]
fn format_takes_config_file_given() {
    if nu_missing() {
        return;
    }
    let dir = TempDir::new();
    let config = dir.file("nuten.nuon", "{commas: trailing}\n");
    let file = dir.file("a.nu", "[\n  a\n  b\n]\n");

    let output = oracle_with_config(&config, &[&file]);

    assert_eq!(output.status.code(), Some(0), "{output:?}");
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(
        stderr.contains("files: 1, failed: 0, skipped: 0"),
        "{stderr}"
    );
}

#[test]
fn format_exits_with_2_when_config_file_cannot_be_loaded() {
    if nu_missing() {
        return;
    }
    let dir = TempDir::new();
    let config = dir.file("nuten.nuon", "{commas: never}\n");
    let file = dir.file("a.nu", "ls\n");

    let output = oracle_with_config(&config, &[&file]);

    assert_eq!(output.status.code(), Some(2), "{output:?}");
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("`commas` takes one of"), "{stderr}");
}

#[test]
fn format_exits_with_2_when_config_file_is_missing() {
    if nu_missing() {
        return;
    }
    let dir = TempDir::new();
    let config = dir.0.join("missing.nuon");
    let file = dir.file("a.nu", "ls\n");

    let output = oracle_with_config(&config, &[&file]);

    assert_eq!(output.status.code(), Some(2), "{output:?}");
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("missing.nuon"), "{stderr}");
}
