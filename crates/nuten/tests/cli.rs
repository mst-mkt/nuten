//! Runs the binary on files in a temporary directory, and with its stdout closed or full.

use std::{
    env,
    ffi::OsStr,
    fs, io,
    path::{Path, PathBuf},
    process::{Command, Output},
    sync::atomic::{AtomicUsize, Ordering},
};

/// A fresh directory for each test, removed when dropped.
struct TempDir(PathBuf);

impl TempDir {
    fn new() -> TempDir {
        static COUNT: AtomicUsize = AtomicUsize::new(0);
        let count = COUNT.fetch_add(1, Ordering::Relaxed);
        let dir = env::temp_dir().join(format!("nuten-cli-{}-{count}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        TempDir(dir)
    }

    fn path(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }

    fn file(&self, name: &str, contents: &str) -> PathBuf {
        let path = self.path(name);
        fs::write(&path, contents).unwrap();
        path
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).ok();
    }
}

fn nuten(args: &[&OsStr]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_nuten"))
        .args(args)
        .output()
        .unwrap()
}

fn format(paths: &[&Path]) -> Output {
    let args: Vec<_> = [OsStr::new("format")]
        .into_iter()
        .chain(paths.iter().map(|path| path.as_os_str()))
        .collect();
    nuten(&args)
}

fn stderr(output: &Output) -> String {
    String::from_utf8(output.stderr.clone()).unwrap()
}

#[test]
fn format_rewrites_each_file_in_place() {
    let dir = TempDir::new();
    let a = dir.file("a.nu", "ls|first");
    let b = dir.file("b.nu", "echo   a\n\n\n");
    let output = format(&[&a, &b]);
    assert_eq!(output.status.code(), Some(0));
    assert!(output.stdout.is_empty());
    assert!(output.stderr.is_empty());
    assert_eq!(fs::read_to_string(&a).unwrap(), "ls | first\n");
    assert_eq!(fs::read_to_string(&b).unwrap(), "echo a\n");
}

#[test]
fn format_leaves_formatted_file_alone() {
    let dir = TempDir::new();
    let path = dir.file("a.nu", "ls | first\n");
    // Writing would fail.
    let mut permissions = fs::metadata(&path).unwrap().permissions();
    permissions.set_readonly(true);
    fs::set_permissions(&path, permissions).unwrap();
    let output = format(&[&path]);
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    assert!(output.stderr.is_empty());
}

#[test]
fn format_reports_syntax_error_position_and_continues() {
    let dir = TempDir::new();
    let bad = dir.file("bad.nu", "ls\n\n  ls || x\n");
    let good = dir.file("good.nu", "ls|first");
    let output = format(&[&bad, &good]);
    assert_eq!(output.status.code(), Some(2));
    let stderr = stderr(&output);
    assert_eq!(stderr.lines().count(), 1, "{stderr}");
    assert!(
        stderr.starts_with(&format!("{}:3:6: ", bad.display())),
        "{stderr}"
    );
    assert_eq!(fs::read_to_string(&bad).unwrap(), "ls\n\n  ls || x\n");
    assert_eq!(fs::read_to_string(&good).unwrap(), "ls | first\n");
}

#[test]
fn format_reports_unreadable_file() {
    let dir = TempDir::new();
    let missing = dir.path("missing.nu");
    let output = format(&[&missing]);
    assert_eq!(output.status.code(), Some(2));
    assert!(stderr(&output).starts_with(&format!("{}: ", missing.display())));
}

#[test]
fn format_wants_at_least_one_file() {
    let output = format(&[]);
    assert_eq!(output.status.code(), Some(2));
    assert!(!output.stderr.is_empty());
}

#[test]
fn format_rejects_unknown_flag() {
    let output = format(&[Path::new("--bogus")]);
    assert_eq!(output.status.code(), Some(2));
    let stderr = stderr(&output);
    assert!(stderr.contains("--bogus"), "{stderr}");
    assert!(!stderr.contains("No such file"), "{stderr}");
}

#[test]
fn format_takes_file_named_like_subcommand() {
    let dir = TempDir::new();
    let path = dir.file("format", "ls|first");
    let output = format(&[&path]);
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    assert_eq!(fs::read_to_string(&path).unwrap(), "ls | first\n");
}

#[test]
fn wants_subcommand() {
    let output = nuten(&[]);
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
}

#[test]
fn rejects_file_without_subcommand() {
    let dir = TempDir::new();
    let path = dir.file("a.nu", "ls|first");
    let output = nuten(&[path.as_os_str()]);
    assert_eq!(output.status.code(), Some(2));
    assert_eq!(fs::read_to_string(&path).unwrap(), "ls|first");
}

#[test]
fn schema_prints_json_schema_of_config_file() {
    let output = nuten(&[OsStr::new("schema")]);
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    assert!(output.stderr.is_empty());
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        nuten_config::schema()
    );
}

#[test]
fn schema_rejects_extra_argument() {
    let output = nuten(&[OsStr::new("schema"), OsStr::new("x")]);
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
}

#[test]
fn schema_succeeds_when_reader_closes_first() {
    let (reader, writer) = io::pipe().unwrap();
    drop(reader);
    let output = Command::new(env!("CARGO_BIN_EXE_nuten"))
        .arg("schema")
        .stdout(writer)
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    assert!(output.stderr.is_empty());
}

#[cfg(target_os = "linux")]
#[test]
fn schema_reports_write_error() {
    let full = fs::File::options().write(true).open("/dev/full").unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_nuten"))
        .arg("schema")
        .stdout(full)
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    assert!(!output.stderr.is_empty());
}
