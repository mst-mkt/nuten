#[cfg(unix)]
use crate::support::stderr;
use crate::support::{
    BLOCK, FORMATTED, TempDir, UNFORMATTED, assert_succeeds_quietly, format_in, nuten_command,
    read, write_unformatted,
};

#[test]
fn format_formats_current_directory_without_paths() {
    let dir = TempDir::new();
    dir.dir(".git");
    dir.file(".gitignore", "gen/\n");
    let [taken, ignored] = write_unformatted(&dir, ["a/a.nu", "gen/a.nu"]);
    let output = format_in(&dir, &[]);
    assert_succeeds_quietly(&output);
    assert_eq!(read(&taken), FORMATTED);
    assert_eq!(read(&ignored), UNFORMATTED);
}

#[cfg(unix)]
#[test]
fn format_reports_files_under_current_directory_without_paths_by_paths_relative_to_it() {
    let dir = TempDir::new();
    dir.file("a/bad.nu", "ls ||");
    let output = format_in(&dir, &[]);
    assert_eq!(output.status.code(), Some(2));
    let stderr = stderr(&output);
    assert!(stderr.starts_with("a/bad.nu:1:4: "), "{stderr}");
}

#[test]
fn format_takes_respect_gitignore_from_config_file_above_current_directory_without_paths() {
    let dir = TempDir::new();
    dir.file("c/nuten.nuon", "{respect_gitignore: false}");
    dir.file("c/d/.ignore", "a.nu\n");
    let [path] = write_unformatted(&dir, ["c/d/a.nu"]);
    let output = nuten_command()
        .arg("format")
        .current_dir(dir.path("c/d"))
        .output()
        .unwrap();
    assert_succeeds_quietly(&output);
    assert_eq!(read(&path), FORMATTED);
}

#[test]
fn format_takes_given_config_file_for_current_directory_without_paths() {
    let dir = TempDir::new();
    dir.file("two.nuon", "{indent_width: 2}");
    let path = dir.file("a.nu", BLOCK);
    let output = format_in(&dir, &["-c", "two.nuon"]);
    assert_succeeds_quietly(&output);
    assert_eq!(read(&path), "if true {\n  ls\n}\n");
}
