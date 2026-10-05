#[cfg(target_os = "linux")]
use std::fs;
use std::{ffi::OsStr, io};

use crate::support::{
    FORMATTED, TempDir, format_in, format_stdin_in, nuten, nuten_command, problems, read, stderr,
    stdout,
};

#[test]
fn format_help_examples_each_succeed_quietly_on_formatted_files() {
    let help = stdout(&format_in(&TempDir::new(), &["--help"]));
    let examples: Vec<_> = help
        .lines()
        .filter_map(|line| line.trim_start().strip_prefix("$ "))
        .collect();
    assert!(!examples.is_empty(), "{help}");
    for example in examples {
        let args: Vec<_> = example
            .strip_prefix("nuten format")
            .unwrap_or_else(|| panic!("{example}: not a run of `nuten format`"))
            .split_whitespace()
            .collect();
        let dir = TempDir::new();
        dir.file("src/a.nu", FORMATTED);
        let output = format_stdin_in(&dir, &args, FORMATTED.as_bytes());
        assert_eq!(
            (output.status.code(), stderr(&output).as_str()),
            (Some(0), ""),
            "{example}"
        );
    }
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
    assert_eq!(read(&path), "ls|first");
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
    let output = nuten_command()
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
    let output = nuten_command().arg("schema").stdout(full).output().unwrap();
    assert_eq!(output.status.code(), Some(2));
    let problems = problems(&output);
    assert!(problems.starts_with("stdout: "), "{problems}");
}
