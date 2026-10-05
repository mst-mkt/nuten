use std::{ffi::OsStr, fs, path::Path};

use crate::support::{BLOCK, TempDir, format_in, io_error_line, nuten, read, stderr};

const NO_CONFIG_EXTENSION: &str =
    "config file needs one of the extensions `.nuon`, `.jsonc`, `.json`";

#[test]
fn format_takes_given_config_file_over_nearest() {
    let dir = TempDir::new();
    dir.file("nuten.nuon", "{indent_width: 3}");
    dir.file("style/two.json", r#"{"indent_width": 2}"#);
    let path = dir.file("a.nu", BLOCK);
    for flag in ["-c", "--config"] {
        fs::write(&path, BLOCK).unwrap();
        let output = format_in(&dir, &[flag, "style/two.json", "a.nu"]);
        assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
        assert_eq!(read(&path), "if true {\n  ls\n}\n", "{flag}");
    }
}

#[test]
fn format_takes_given_config_file_for_every_file() {
    let dir = TempDir::new();
    dir.file("a/nuten.nuon", "{indent_width: 3}");
    let config = dir.file("two.nuon", "{indent_width: 2}");
    let paths = ["a.nu", "a/a.nu", "b/a.nu"].map(|name| dir.file(name, BLOCK));
    let args: Vec<_> = [OsStr::new("format"), OsStr::new("-c"), config.as_os_str()]
        .into_iter()
        .chain(paths.iter().map(|path| path.as_os_str()))
        .collect();
    let output = nuten(&args);
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    for path in &paths {
        assert_eq!(read(path), "if true {\n  ls\n}\n", "{}", path.display());
    }
}

#[test]
fn format_leaves_wrong_nearest_config_file_unread_when_given_one() {
    let dir = TempDir::new();
    dir.file("nuten.nuon", "{");
    dir.file("two.nuon", "{indent_width: 2}");
    let path = dir.file("a.nu", BLOCK);
    let output = format_in(&dir, &["-c", "two.nuon", "a.nu"]);
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    assert_eq!(read(&path), "if true {\n  ls\n}\n");
}

#[test]
fn format_writes_nothing_when_given_config_file_cannot_be_read() {
    let dir = TempDir::new();
    dir.dir("dir.json");
    let path = dir.file("a.nu", "ls|first");
    let missing_error = fs::read(dir.path("missing.nuon")).unwrap_err();
    for (config, expected) in [
        (
            "missing.nuon",
            format!(
                "error: {}",
                io_error_line(Path::new("missing.nuon"), &missing_error)
            ),
        ),
        (
            "dir.json",
            "error: dir.json: not a regular file\n".to_string(),
        ),
    ] {
        let output = format_in(&dir, &["-c", config, "a.nu"]);
        assert_eq!(output.status.code(), Some(2), "{config}");
        assert_eq!(stderr(&output), expected);
        assert_eq!(read(&path), "ls|first", "{config}");
    }
}

#[test]
fn format_rejects_given_config_file_without_config_extension() {
    let dir = TempDir::new();
    let path = dir.file("a.nu", "ls|first");
    for config in ["nuten.toml", "nuten", "nuten.JSON"] {
        dir.file(config, "{}");
        let output = format_in(&dir, &["-c", config, "a.nu"]);
        assert_eq!(output.status.code(), Some(2), "{config}");
        assert_eq!(
            stderr(&output),
            format!("error: {config}: {NO_CONFIG_EXTENSION}\n")
        );
        assert_eq!(read(&path), "ls|first", "{config}");
    }
}

#[test]
fn format_writes_nothing_when_given_config_file_is_wrong() {
    let dir = TempDir::new();
    dir.file("two.nuon", "{indent_width: 2, a: 1}");
    let path = dir.file("a.nu", "ls|first");
    let output = format_in(&dir, &["-c", "two.nuon", "a.nu"]);
    assert_eq!(output.status.code(), Some(2));
    assert_eq!(stderr(&output), "error: two.nuon:1:19: unknown key `a`\n");
    assert_eq!(read(&path), "ls|first");
}

#[test]
fn format_wants_value_for_config_flag() {
    let dir = TempDir::new();
    let path = dir.file("a.nu", "ls|first");
    let output = format_in(&dir, &["a.nu", "-c"]);
    assert_eq!(output.status.code(), Some(2));
    assert!(!output.stderr.is_empty());
    assert_eq!(read(&path), "ls|first");
}

#[test]
fn format_rejects_repeated_config_flag() {
    let dir = TempDir::new();
    dir.file("two.nuon", "{indent_width: 2}");
    dir.file("three.nuon", "{indent_width: 3}");
    let path = dir.file("a.nu", BLOCK);
    let output = format_in(&dir, &["-c", "two.nuon", "--config", "three.nuon", "a.nu"]);
    assert_eq!(output.status.code(), Some(2));
    let stderr = stderr(&output);
    assert_eq!(
        stderr.lines().next(),
        Some("error: the argument '--config' cannot be used multiple times"),
        "{stderr}"
    );
    assert_eq!(read(&path), BLOCK);
}

#[test]
fn format_takes_given_config_for_other_files_when_one_has_syntax_error() {
    let dir = TempDir::new();
    dir.file("two.nuon", "{indent_width: 2}");
    let bad = dir.file("bad.nu", "ls ||");
    let good = dir.file("good.nu", BLOCK);
    let output = format_in(&dir, &["-c", "two.nuon", "bad.nu", "good.nu"]);
    assert_eq!(output.status.code(), Some(2));
    assert_eq!(read(&bad), "ls ||");
    assert_eq!(read(&good), "if true {\n  ls\n}\n");
}
