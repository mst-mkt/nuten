#[cfg(target_os = "linux")]
use std::ffi::OsStr;
use std::{fs, path::PathBuf};

#[cfg(target_os = "linux")]
use crate::support::nuten_command;
use crate::support::{BLOCK, TempDir, format, format_in, io_error_line, problems, read, stderr};
#[cfg(unix)]
use crate::support::{fifo, format_in_time};

#[test]
fn format_takes_config_file_in_directory_of_file() {
    let dir = TempDir::new();
    dir.file("nuten.nuon", "{indent_width: 2}");
    let path = dir.file("a.nu", BLOCK);
    let output = format(&[&path]);
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    assert_eq!(read(&path), "if true {\n  ls\n}\n");
}

#[test]
fn format_takes_config_file_in_ancestor_directory() {
    let dir = TempDir::new();
    dir.file("nuten.jsonc", "// two\n{\"indent_width\": 2,}");
    let path = dir.file("a/b/a.nu", BLOCK);
    let output = format(&[&path]);
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    assert_eq!(read(&path), "if true {\n  ls\n}\n");
}

#[test]
fn format_takes_nearest_config_file_for_each_file() {
    let dir = TempDir::new();
    dir.file("nuten.json", r#"{"indent_width": 2}"#);
    dir.file("a/nuten.json", r#"{"indent_width": 3}"#);
    let top = dir.file("a.nu", BLOCK);
    let inner = dir.file("a/a.nu", BLOCK);
    let bare = dir.file("b/a.nu", BLOCK);
    let output = format(&[&top, &inner, &bare]);
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    assert_eq!(read(&top), "if true {\n  ls\n}\n");
    assert_eq!(read(&inner), "if true {\n   ls\n}\n");
    assert_eq!(read(&bare), "if true {\n  ls\n}\n");
}

#[test]
fn format_takes_nuon_over_jsonc_and_json_in_same_directory() {
    let dir = TempDir::new();
    dir.file("nuten.nuon", "{indent_width: 2}");
    dir.file("nuten.jsonc", r#"{"indent_width": 3}"#);
    dir.file("nuten.json", r#"{"indent_width": 5}"#);
    let path = dir.file("a.nu", BLOCK);
    let output = format(&[&path]);
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    assert_eq!(read(&path), "if true {\n  ls\n}\n");
}

#[test]
fn format_takes_defaults_when_config_file_is_only_in_subdirectory() {
    let dir = TempDir::new();
    dir.file("a/nuten.nuon", "{indent_width: 2}");
    let path = dir.file("a.nu", BLOCK);
    let output = format(&[&path]);
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    assert_eq!(read(&path), "if true {\n    ls\n}\n");
}

#[test]
fn format_takes_defaults_when_config_file_is_only_above_repository_root() {
    let dir = TempDir::new();
    dir.file("nuten.nuon", "{indent_width: 2}");
    dir.dir("a/.git");
    let path = dir.file("a/b/a.nu", BLOCK);
    let output = format(&[&path]);
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    assert_eq!(read(&path), "if true {\n    ls\n}\n");
}

#[test]
fn format_looks_for_config_file_from_current_directory_for_bare_file_name() {
    let dir = TempDir::new();
    dir.file("nuten.nuon", "{indent_width: 2}");
    let path = dir.file("a.nu", BLOCK);
    let output = format_in(&dir, &["a.nu"]);
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    assert_eq!(read(&path), "if true {\n  ls\n}\n");
}

#[test]
fn format_looks_for_config_file_where_dot_dot_leads() {
    let dir = TempDir::new();
    dir.file("a/nuten.nuon", "{indent_width: 2}");
    let path = dir.file("a.nu", BLOCK);
    let output = format_in(&dir, &["a/../a.nu"]);
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    assert_eq!(read(&path), "if true {\n    ls\n}\n");
}

#[cfg(unix)]
#[test]
fn format_looks_for_config_file_where_symlinked_directory_leads() {
    let dir = TempDir::new();
    dir.file("a/nuten.nuon", "{indent_width: 2}");
    let path = dir.file("a/real/a.nu", BLOCK);
    std::os::unix::fs::symlink(dir.path("a/real"), dir.dir("b").join("link")).unwrap();
    let output = format_in(&dir, &["b/link/a.nu"]);
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    assert_eq!(read(&path), "if true {\n  ls\n}\n");
}

#[cfg(unix)]
#[test]
fn format_looks_for_config_file_where_symlinked_file_leads() {
    let dir = TempDir::new();
    dir.file("a/nuten.nuon", "{indent_width: 2}");
    dir.file("b/nuten.nuon", "{indent_width: 3}");
    let path = dir.file("a/a.nu", BLOCK);
    std::os::unix::fs::symlink(&path, dir.path("b/link.nu")).unwrap();
    let output = format_in(&dir, &["b/link.nu"]);
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    assert_eq!(read(&path), "if true {\n  ls\n}\n");
}

#[test]
fn format_takes_config_for_other_files_when_one_has_syntax_error() {
    let dir = TempDir::new();
    dir.file("nuten.nuon", "{indent_width: 2}");
    let bad = dir.file("bad.nu", "ls ||");
    let good = dir.file("good.nu", BLOCK);
    let output = format(&[&bad, &good]);
    assert_eq!(output.status.code(), Some(2));
    assert_eq!(read(&bad), "ls ||");
    assert_eq!(read(&good), "if true {\n  ls\n}\n");
}

#[test]
fn format_writes_nothing_when_any_config_file_is_wrong() {
    let dir = TempDir::new();
    dir.file("a/nuten.json", "{\n  \"indent_width\": 0\n}");
    let wrong = dir.file("a/a.nu", "ls|first");
    let right = dir.file("b/a.nu", "ls|first");
    let output = format(&[&right, &wrong]);
    assert_eq!(output.status.code(), Some(2));
    assert_eq!(
        problems(&output),
        format!(
            "{}:2:19: `indent_width` takes an integer from 1 to 255\n",
            dir.path("a/nuten.json").display()
        )
    );
    assert_eq!(read(&wrong), "ls|first");
    assert_eq!(read(&right), "ls|first");
}

#[test]
fn format_reports_every_error_in_config_file() {
    let dir = TempDir::new();
    let config = dir.file("nuten.nuon", "{indent_width: 0, a: 1}");
    let path = dir.file("a.nu", "ls|first");
    let output = format(&[&path]);
    assert_eq!(output.status.code(), Some(2));
    assert_eq!(
        problems(&output),
        format!(
            "{0}:1:16: `indent_width` takes an integer from 1 to 255\n\
             {0}:1:19: unknown key `a`\n",
            config.display()
        )
    );
}

#[test]
fn format_reports_config_error_line_in_crlf_file() {
    let dir = TempDir::new();
    let config = dir.file("nuten.json", "{\r\n  \"indent_width\": 0\r\n}");
    let path = dir.file("a.nu", "ls|first");
    let output = format(&[&path]);
    assert_eq!(output.status.code(), Some(2));
    assert_eq!(
        problems(&output),
        format!(
            "{}:2:19: `indent_width` takes an integer from 1 to 255\n",
            config.display()
        )
    );
}

#[test]
fn format_reports_each_wrong_config_file_once() {
    let dir = TempDir::new();
    let nuon = dir.file("a/nuten.nuon", "{a: 1}");
    let json = dir.file("b/nuten.json", "{");
    let paths = ["a/x.nu", "b/x.nu", "a/y.nu", "b/c/x.nu"].map(|name| dir.file(name, "ls"));
    let output = format(&paths.each_ref().map(PathBuf::as_path));
    assert_eq!(output.status.code(), Some(2));
    assert_eq!(
        problems(&output),
        format!(
            "{}:1:2: unknown key `a`\n{}:1:1: Unterminated object\n",
            nuon.display(),
            json.display()
        )
    );
}

#[test]
fn format_does_not_fall_back_from_wrong_config_file_to_next_name() {
    let dir = TempDir::new();
    let nuon = dir.file("nuten.nuon", "{indent_width: 2");
    dir.file("nuten.json", r#"{"indent_width": 2}"#);
    let path = dir.file("a.nu", BLOCK);
    let output = format(&[&path]);
    assert_eq!(output.status.code(), Some(2));
    assert_eq!(
        problems(&output),
        format!("{}:1:1: unclosed quote or bracket\n", nuon.display())
    );
    assert_eq!(read(&path), BLOCK);
}

#[cfg(unix)]
#[test]
fn format_does_not_fall_back_from_dangling_symlink_config_file() {
    let dir = TempDir::new();
    let nuon = dir.path("nuten.nuon");
    std::os::unix::fs::symlink(dir.path("missing"), &nuon).unwrap();
    dir.file("nuten.json", r#"{"indent_width": 2}"#);
    let path = dir.file("a.nu", BLOCK);
    let output = format(&[&path]);
    assert_eq!(output.status.code(), Some(2));
    let error = fs::read(&nuon).unwrap_err();
    assert_eq!(problems(&output), io_error_line(&nuon, &error));
    assert_eq!(read(&path), BLOCK);
}

#[test]
fn format_writes_nothing_when_config_file_is_directory() {
    let dir = TempDir::new();
    let nuon = dir.dir("nuten.nuon");
    let path = dir.file("a.nu", "ls|first");
    let output = format(&[&path]);
    assert_eq!(output.status.code(), Some(2));
    assert_eq!(
        problems(&output),
        format!("{}: not a regular file\n", nuon.display())
    );
    assert_eq!(read(&path), "ls|first");
}

#[cfg(unix)]
#[test]
fn format_writes_nothing_without_waiting_when_config_file_is_fifo() {
    let dir = TempDir::new();
    let nuon = dir.path("nuten.nuon");
    fifo(&nuon);
    let path = dir.file("a.nu", "ls|first");
    let output = format_in_time(&[&path]);
    assert_eq!(output.status.code(), Some(2));
    assert_eq!(
        problems(&output),
        format!("{}: not a regular file\n", nuon.display())
    );
    assert_eq!(read(&path), "ls|first");
}

#[cfg(target_os = "linux")]
#[test]
fn format_writes_nothing_when_existence_of_config_file_cannot_be_told() {
    const PATH_MAX: usize = 4096;
    let dir = TempDir::new();
    // A directory whose path leaves room for `/a.nu` but not for `/nuten.nuon`.
    let deep_length = PATH_MAX - 8;
    // Components of 99 bytes, since one longer than NAME_MAX cannot be created.
    let relative: String = (0..deep_length - dir.0.as_os_str().len() - 1)
        .map(|index| if index % 100 == 99 { '/' } else { 'd' })
        .collect();
    let deep = dir.dir(relative.trim_end_matches('/'));
    let paths = ["a.nu", "b.nu"].map(|name| deep.join(name));
    for path in &paths {
        fs::write(path, "ls|first").unwrap();
    }
    let good = dir.file("b.nu", "ls|first");
    let nuon = deep.join("nuten.nuon");
    let error = fs::symlink_metadata(&nuon).unwrap_err();
    // Two files in one directory, so that it is looked up and reported once for both.
    let output = nuten_command()
        .args([
            OsStr::new("format"),
            OsStr::new("a.nu"),
            OsStr::new("b.nu"),
            good.as_os_str(),
        ])
        .current_dir(&deep)
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    assert_eq!(problems(&output), io_error_line(&nuon, &error));
    for path in &paths {
        assert_eq!(read(path), "ls|first");
    }
    assert_eq!(read(&good), "ls|first");
}

#[test]
fn format_reports_file_in_missing_directory_and_continues() {
    let dir = TempDir::new();
    let missing = dir.path("missing/a.nu");
    let good = dir.file("a.nu", "ls|first");
    let output = format(&[&missing, &good]);
    assert_eq!(output.status.code(), Some(2));
    let error = fs::read(&missing).unwrap_err();
    assert_eq!(problems(&output), io_error_line(&missing, &error));
    assert_eq!(read(&good), "ls | first\n");
}

#[test]
fn format_reports_file_under_regular_file_and_continues() {
    let dir = TempDir::new();
    dir.file("a", "");
    let under = dir.path("a/a.nu");
    let good = dir.file("b.nu", "ls|first");
    let output = format(&[&under, &good]);
    assert_eq!(output.status.code(), Some(2));
    let error = fs::read(&under).unwrap_err();
    assert_eq!(problems(&output), io_error_line(&under, &error));
    assert_eq!(read(&good), "ls | first\n");
}

#[cfg(unix)]
#[test]
fn format_reports_file_in_unsearchable_directory_alongside_wrong_config_file() {
    use std::os::unix::fs::PermissionsExt;

    let dir = TempDir::new();
    let locked = dir.file("a/a.nu", "ls|first");
    let json = dir.file("b/nuten.json", "{");
    let other = dir.file("b/a.nu", "ls|first");
    fs::set_permissions(dir.path("a"), fs::Permissions::from_mode(0o000)).unwrap();
    let error = fs::read(&locked).err();
    let output = format(&[&locked, &other]);
    fs::set_permissions(dir.path("a"), fs::Permissions::from_mode(0o755)).unwrap();
    // Root searches a directory without permission, so the test proves nothing then.
    let Some(error) = error else {
        return;
    };
    assert_eq!(output.status.code(), Some(2));
    assert_eq!(
        problems(&output),
        io_error_line(&locked, &error) + &format!("{}:1:1: Unterminated object\n", json.display())
    );
    assert_eq!(read(&locked), "ls|first");
    assert_eq!(read(&other), "ls|first");
}
