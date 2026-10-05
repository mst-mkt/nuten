use std::{
    ffi::OsStr,
    fs,
    path::Path,
    process::{Command, Output, Stdio},
};

use crate::support::{
    BLOCK, TempDir, UNFORMATTED, assert_succeeds_quietly, problems, read, stdout,
};

/// The shell removes `removed` before it runs the binary, which `Command` cannot.
#[cfg(unix)]
fn nuten_in_removed(removed: &Path, args: &[&OsStr], stdin: Stdio) -> Output {
    Command::new("sh")
        .args([
            "-c",
            r#"cd "$1" && rmdir "$1" && shift 2 && exec "$0" "$@""#,
        ])
        .arg(env!("CARGO_BIN_EXE_nuten"))
        .arg(removed)
        .arg("--")
        .args(args)
        .stdin(stdin)
        .output()
        .unwrap()
}

#[cfg(unix)]
#[test]
fn format_takes_given_config_file_whose_patterns_need_no_directory_in_removed_current_directory() {
    let dir = TempDir::new();
    let config = dir.file("nuten.nuon", r#"{include: ["*.nu"], indent_width: 2}"#);
    let path = dir.file("a.nu", BLOCK);
    let args = [
        OsStr::new("format"),
        OsStr::new("-c"),
        config.as_os_str(),
        path.as_os_str(),
    ];
    let output = nuten_in_removed(&dir.dir("removed"), &args, Stdio::null());
    assert_succeeds_quietly(&output);
    assert_eq!(read(&path), "if true {\n  ls\n}\n");
}

#[cfg(unix)]
#[test]
fn format_reports_removed_current_directory_given_config_file_takes_patterns_from() {
    let dir = TempDir::new();
    let config = dir.file("nuten.nuon", r#"{exclude: ["vendor"]}"#);
    let path = dir.file("a.nu", UNFORMATTED);
    let args = [
        OsStr::new("format"),
        OsStr::new("-c"),
        config.as_os_str(),
        path.as_os_str(),
    ];
    let output = nuten_in_removed(&dir.dir("removed"), &args, Stdio::null());
    assert_eq!(output.status.code(), Some(2));
    let expected = format!(
        "{}: its patterns are relative to the current directory, which cannot be resolved: ",
        config.display()
    );
    assert!(
        problems(&output).starts_with(&expected),
        "{}",
        problems(&output)
    );
    assert_eq!(read(&path), UNFORMATTED);
}

#[cfg(unix)]
#[test]
fn format_takes_given_config_file_without_exclude_for_stdin_in_removed_current_directory() {
    let dir = TempDir::new();
    let config = dir.file("nuten.nuon", "{indent_width: 2}");
    let input = dir.file("stdin.input", BLOCK);
    let args = [
        OsStr::new("format"),
        OsStr::new("-c"),
        config.as_os_str(),
        OsStr::new("--stdin-filepath"),
        OsStr::new("a.nu"),
        OsStr::new("-"),
    ];
    let stdin = Stdio::from(fs::File::open(&input).unwrap());
    let output = nuten_in_removed(&dir.dir("removed"), &args, stdin);
    assert_succeeds_quietly(&output);
    assert_eq!(stdout(&output), "if true {\n  ls\n}\n");
}
