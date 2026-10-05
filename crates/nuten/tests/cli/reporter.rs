use std::path::Path;

use crate::support::{
    FORMATTED, TempDir, UNFORMATTED, format_in, format_stdin_in, problems, read, stderr, stdout,
    write_unformatted,
};

/// The `file` of an annotation names `path` as written, with its `:` escaped as GitHub asks.
fn file_property(path: &Path) -> String {
    path.display().to_string().replace(':', "%3A")
}

fn file_annotation(path: &Path, lines: &str) -> String {
    format!(
        "::error file={},{lines},title=nuten::Would be reformatted\n",
        file_property(path)
    )
}

#[test]
fn format_reporter_github_annotates_lines_of_each_file_that_would_change_and_changes_none() {
    let dir = TempDir::new();
    let [a] = write_unformatted(&dir, ["a.nu"]);
    let b = dir.file("b.nu", "ls\nls\nls|x\nls\nls|y\n");
    let formatted = dir.file("c.nu", FORMATTED);
    let paths = [&a, &b, &formatted].map(|path| path.to_str().unwrap().to_string());
    let args: Vec<_> = ["--reporter", "github"]
        .into_iter()
        .chain(paths.iter().map(String::as_str))
        .collect();
    let output = format_in(&dir, &args);
    assert_eq!(output.status.code(), Some(1), "{}", stderr(&output));
    assert_eq!(
        stdout(&output),
        file_annotation(&a, "line=1,endLine=1") + &file_annotation(&b, "line=3,endLine=5")
    );
    assert_eq!(
        stderr(&output),
        "3 files: 2 need formatting (3 lines, 3 whitespace only), 1 unchanged\n"
    );
    assert_eq!(read(&a), UNFORMATTED);
    assert_eq!(read(&formatted), FORMATTED);
}

#[test]
fn format_reporter_github_annotates_each_problem_at_its_position_and_reports_it_on_stderr() {
    let dir = TempDir::new();
    let bad = dir.file("bad.nu", "ls\nls ||");
    let output = format_in(&dir, &["--reporter", "github", bad.to_str().unwrap()]);
    assert_eq!(output.status.code(), Some(2));
    let annotation = stdout(&output);
    let place = format!(
        "::error file={},line=2,col=4,title=nuten::",
        file_property(&bad)
    );
    assert!(annotation.starts_with(&place), "{annotation}");
    assert_eq!(annotation.lines().count(), 1, "{annotation}");
    let problems = problems(&output);
    assert!(
        problems.starts_with(&format!("{}:2:4: ", bad.display())),
        "{problems}"
    );
}

#[test]
fn format_reporter_github_annotates_stdin_as_file_named_by_stdin_filepath() {
    let dir = TempDir::new();
    let path = dir.path("a.nu");
    let output = format_stdin_in(
        &dir,
        &[
            "--reporter",
            "github",
            "--stdin-filepath",
            path.to_str().unwrap(),
            "-",
        ],
        UNFORMATTED.as_bytes(),
    );
    assert_eq!(output.status.code(), Some(1), "{}", stderr(&output));
    assert_eq!(stdout(&output), file_annotation(&path, "line=1,endLine=1"));
    assert!(output.stderr.is_empty(), "{}", stderr(&output));
}

#[test]
fn format_reporter_github_on_stdin_needs_stdin_filepath() {
    let dir = TempDir::new();
    let output = format_stdin_in(&dir, &["--reporter", "github", "-"], UNFORMATTED.as_bytes());
    assert_eq!(output.status.code(), Some(2));
    let problem =
        "`--reporter` needs `--stdin-filepath` on stdin, which names the file to report on";
    assert_eq!(stdout(&output), format!("::error title=nuten::{problem}\n"));
    assert_eq!(problems(&output), format!("{problem}\n"));
}

#[test]
fn format_reporter_github_annotates_problem_of_given_config_file() {
    let dir = TempDir::new();
    let [path] = write_unformatted(&dir, ["a.nu"]);
    let config = dir.file("c.nuon", "{nope: 1}");
    let output = format_in(
        &dir,
        &[
            "--reporter",
            "github",
            "-c",
            config.to_str().unwrap(),
            path.to_str().unwrap(),
        ],
    );
    assert_eq!(output.status.code(), Some(2));
    assert_eq!(
        stdout(&output),
        format!(
            "::error file={},line=1,col=2,title=nuten::unknown key `nope`\n",
            file_property(&config)
        )
    );
    assert_eq!(read(&path), UNFORMATTED);
}

#[test]
fn format_reporter_cannot_be_used_with_check_or_list_different() {
    let dir = TempDir::new();
    let [path] = write_unformatted(&dir, ["a.nu"]);
    for flag in ["--check", "--list-different"] {
        let output = format_in(&dir, &["--reporter", "github", flag, "a.nu"]);
        assert_eq!(output.status.code(), Some(2), "{flag}");
        let stderr = stderr(&output);
        assert!(stderr.contains("cannot be used with"), "{flag}: {stderr}");
    }
    assert_eq!(read(&path), UNFORMATTED);
}
