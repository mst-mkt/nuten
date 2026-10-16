use std::path::MAIN_SEPARATOR;

use crate::support::{
    FORMATTED, TempDir, UNFORMATTED, format_in, format_stdin_in, read, stderr, stdout,
    write_unformatted,
};

#[test]
fn format_list_different_lists_files_it_would_change_in_order_given_and_changes_none() {
    let dir = TempDir::new();
    let [z, a] = write_unformatted(&dir, ["z.nu", "d/a.nu"]);
    let formatted = dir.file("b.nu", FORMATTED);
    for flag in ["--list-different", "-l"] {
        let output = format_in(&dir, &[flag, "z.nu", "b.nu", "d"]);
        assert_eq!(output.status.code(), Some(1), "{flag}: {}", stderr(&output));
        assert_eq!(
            stdout(&output),
            format!("z.nu\nd{MAIN_SEPARATOR}a.nu\n"),
            "{flag}"
        );
    }
    assert_eq!(read(&z), UNFORMATTED);
    assert_eq!(read(&a), UNFORMATTED);
    assert_eq!(read(&formatted), FORMATTED);
}

#[test]
fn format_list_different_succeeds_with_nothing_when_no_file_would_change() {
    let dir = TempDir::new();
    dir.file("a.nu", FORMATTED);
    let output = format_in(&dir, &["--list-different", "a.nu"]);
    assert_eq!(output.status.code(), Some(0));
    assert_eq!(stdout(&output), "");
    assert!(output.stderr.is_empty(), "{}", stderr(&output));
}

#[test]
fn format_list_different_ends_with_no_summary() {
    let dir = TempDir::new();
    write_unformatted(&dir, ["a.nu"]);
    dir.file("bad.nu", "ls || x\n");
    let changed = format_in(&dir, &["--list-different", "a.nu"]);
    assert!(changed.stderr.is_empty(), "{}", stderr(&changed));
    let failed = format_in(&dir, &["--list-different", "a.nu", "bad.nu"]);
    assert_eq!(failed.status.code(), Some(2));
    let stderr = stderr(&failed);
    assert!(
        stderr.lines().all(|line| line.starts_with("error: ")),
        "{stderr}"
    );
}

#[test]
fn format_list_different_names_stdin_that_would_change() {
    let dir = TempDir::new();
    let unnamed = format_stdin_in(&dir, &["--list-different", "-"], b"ls|first");
    assert_eq!(unnamed.status.code(), Some(1));
    assert_eq!(stdout(&unnamed), "<stdin>\n");
    let named = format_stdin_in(
        &dir,
        &["--list-different", "--stdin-filepath", "a.nu", "-"],
        b"ls|first",
    );
    assert_eq!(named.status.code(), Some(1));
    assert_eq!(stdout(&named), "a.nu\n");
}

#[test]
fn format_list_different_names_no_stdin_that_would_not_change() {
    let dir = TempDir::new();
    let output = format_stdin_in(&dir, &["--list-different", "-"], FORMATTED.as_bytes());
    assert_eq!(output.status.code(), Some(0));
    assert_eq!(stdout(&output), "");
}

#[test]
fn format_list_different_names_no_stdin_it_cannot_format() {
    let dir = TempDir::new();
    let output = format_stdin_in(&dir, &["--list-different", "-"], b"ls || x\n");
    assert_eq!(output.status.code(), Some(2));
    assert_eq!(stdout(&output), "");
}

#[test]
fn format_list_different_lists_files_that_would_change_alongside_problems() {
    let dir = TempDir::new();
    write_unformatted(&dir, ["a.nu"]);
    dir.file("bad.nu", "ls || x\n");
    let output = format_in(&dir, &["--list-different", "a.nu", "bad.nu"]);
    assert_eq!(output.status.code(), Some(2));
    assert_eq!(stdout(&output), "a.nu\n");
}

#[test]
fn format_rejects_list_different_with_check() {
    let dir = TempDir::new();
    let [path] = write_unformatted(&dir, ["a.nu"]);
    let output = format_in(&dir, &["--list-different", "--check", "a.nu"]);
    assert_eq!(output.status.code(), Some(2));
    let stderr = stderr(&output);
    assert!(
        stderr.starts_with("error: the argument '--list-different' cannot be used with '--check'"),
        "{stderr}"
    );
    assert_eq!(read(&path), UNFORMATTED);
}
