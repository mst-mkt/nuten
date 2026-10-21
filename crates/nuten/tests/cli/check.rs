#[cfg(target_os = "linux")]
use std::fs;
use std::{io, path::MAIN_SEPARATOR};

use crate::support::{
    FORMATTED, TempDir, UNFORMATTED, assert_succeeds, format_in, nuten_command, problems, read,
    stderr, stdout, view_of_unformatted, write_unformatted,
};

#[test]
fn format_check_shows_files_it_would_change_in_order_given_and_changes_none() {
    let dir = TempDir::new();
    let [z, a, c] = write_unformatted(&dir, ["z.nu", "d/a.nu", "d/c.nuon"]);
    let formatted = dir.file("b.nu", FORMATTED);
    let output = format_in(&dir, &["--check", "z.nu", "b.nu", "d"]);
    assert_eq!(output.status.code(), Some(1), "{}", stderr(&output));
    assert!(problems(&output).is_empty(), "{}", stderr(&output));
    let views = [
        view_of_unformatted("z.nu"),
        view_of_unformatted(&format!("d{MAIN_SEPARATOR}a.nu")),
        view_of_unformatted(&format!("d{MAIN_SEPARATOR}c.nuon")),
    ];
    assert_eq!(stdout(&output), views.concat());
    assert_eq!(read(&z), UNFORMATTED);
    assert_eq!(read(&a), UNFORMATTED);
    assert_eq!(read(&c), UNFORMATTED);
    assert_eq!(read(&formatted), FORMATTED);
}

#[test]
fn format_check_succeeds_and_shows_nothing_when_every_file_is_formatted() {
    let dir = TempDir::new();
    dir.file("a.nu", FORMATTED);
    dir.file("nuten.nuon", "{ indent_width: 2 }\n");
    dir.file("b.nu", "if true {\n  ls\n}\n");
    let output = format_in(&dir, &["--check"]);
    assert_succeeds(&output);
    assert_eq!(stdout(&output), "");
}

#[test]
fn format_check_exits_with_2_on_problem_though_files_would_change() {
    let dir = TempDir::new();
    let bad = dir.file("bad.nu", "ls ||");
    let [path] = write_unformatted(&dir, ["a.nu"]);
    let output = format_in(&dir, &["--check", "bad.nu", "a.nu"]);
    assert_eq!(output.status.code(), Some(2));
    assert_eq!(stdout(&output), view_of_unformatted("a.nu"));
    assert!(
        problems(&output).starts_with("bad.nu:1:4: "),
        "{}",
        problems(&output)
    );
    assert_eq!(read(&bad), "ls ||");
    assert_eq!(read(&path), UNFORMATTED);
}

#[test]
fn format_check_shows_nothing_when_any_config_file_is_wrong() {
    let dir = TempDir::new();
    dir.file("bad/nuten.nuon", "{a: 1}");
    let paths = write_unformatted(&dir, ["bad/a.nu", "ok/b.nu"]);
    for args in [["--check", "bad", "ok"], ["--check", "ok/b.nu", "bad"]] {
        let output = format_in(&dir, &args);
        assert_eq!(output.status.code(), Some(2), "{args:?}");
        assert!(output.stdout.is_empty(), "{args:?}: {}", stdout(&output));
    }
    for path in &paths {
        assert_eq!(read(path), UNFORMATTED, "{}", path.display());
    }
}

#[test]
fn format_check_counts_view_as_written_when_reader_closes_first() {
    let dir = TempDir::new();
    write_unformatted(&dir, ["a.nu"]);
    let (reader, writer) = io::pipe().unwrap();
    drop(reader);
    let output = nuten_command()
        .args(["format", "--check", "a.nu"])
        .current_dir(&dir.0)
        .stdout(writer)
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1), "{}", stderr(&output));
    assert!(problems(&output).is_empty(), "{}", stderr(&output));
}

#[cfg(target_os = "linux")]
#[test]
fn format_check_reports_view_it_cannot_write() {
    let dir = TempDir::new();
    let [path] = write_unformatted(&dir, ["a.nu"]);
    let full = fs::File::options().write(true).open("/dev/full").unwrap();
    let output = nuten_command()
        .args(["format", "--check", "a.nu"])
        .current_dir(&dir.0)
        .stdout(full)
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    let problems = problems(&output);
    assert!(problems.starts_with("stdout: "), "{problems}");
    assert_eq!(read(&path), UNFORMATTED);
}

#[test]
fn format_check_shows_view_in_color_when_clicolor_force_is_set() {
    let dir = TempDir::new();
    dir.file("a.nu", UNFORMATTED);
    let output = nuten_command()
        .args(["format", "--check", "a.nu"])
        .current_dir(&dir.0)
        .env("CLICOLOR_FORCE", "1")
        .output()
        .unwrap();
    let stdout = stdout(&output);
    assert!(stdout.starts_with("\u{1b}[2m╭─ "), "{stdout:?}");
}
