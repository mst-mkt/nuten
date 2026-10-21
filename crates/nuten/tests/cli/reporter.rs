use std::{
    collections::HashSet,
    path::{MAIN_SEPARATOR, Path},
};

use serde_json::{Value, json};

use crate::support::{
    FORMATTED, TempDir, UNFORMATTED, format_in, format_stdin_in, nuten_command, problems, read,
    stderr, stdout, write_unformatted,
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
fn format_reporter_github_annotates_problem_of_editorconfig_that_cannot_be_read() {
    let dir = TempDir::new();
    let editorconfig = dir.dir("d/.editorconfig");
    let path = dir.file("d/a.nu", UNFORMATTED);
    let output = format_in(&dir, &["--reporter", "github", path.to_str().unwrap()]);
    assert_eq!(output.status.code(), Some(2));
    assert_eq!(
        stdout(&output),
        format!(
            "::error file={},title=nuten::not a regular file\n",
            file_property(&editorconfig)
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

fn issues(stdout: &str) -> Vec<Value> {
    serde_json::from_str(stdout).unwrap()
}

fn location(issue: &Value) -> Value {
    json!([
        issue["check_name"],
        issue["severity"],
        issue["location"]["path"],
        issue["location"]["lines"]["begin"],
        issue["location"]["lines"]["end"],
    ])
}

#[test]
fn format_reporter_gitlab_lists_issue_for_each_file_that_would_change_and_each_problem() {
    let dir = TempDir::new();
    let [a] = write_unformatted(&dir, ["d/a.nu"]);
    dir.file("d/b.nu", "ls\nls\nls|x\nls\nls|y\n");
    dir.file("d/c.nu", FORMATTED);
    dir.file("d/bad.nu", "ls\nls ||");
    let output = format_in(&dir, &["--reporter", "gitlab", "d"]);
    assert_eq!(output.status.code(), Some(2), "{}", stderr(&output));
    let issues = issues(&stdout(&output));
    let locations: Vec<_> = issues.iter().map(location).collect();
    assert_eq!(issues[0]["description"], "Would be reformatted");
    assert_eq!(
        locations,
        [
            json!(["nuten/format", "minor", "d/a.nu", 1, 1]),
            json!(["nuten/format", "minor", "d/b.nu", 3, 5]),
            json!(["nuten/problem", "major", "d/bad.nu", 2, 2]),
        ]
    );
    let fingerprints: HashSet<_> = issues
        .iter()
        .map(|issue| issue["fingerprint"].to_string())
        .collect();
    assert_eq!(fingerprints.len(), issues.len(), "{fingerprints:?}");
    let problems = problems(&output);
    assert!(
        problems.starts_with(&format!("d{MAIN_SEPARATOR}bad.nu:2:4: ")),
        "{problems}"
    );
    assert_eq!(read(&a), UNFORMATTED);
}

#[test]
fn format_reporter_gitlab_lists_no_issue_when_nothing_would_change() {
    let dir = TempDir::new();
    dir.file("a.nu", FORMATTED);
    let output = format_in(&dir, &["--reporter", "gitlab", "a.nu"]);
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    assert_eq!(stdout(&output), "[]\n");
}

#[test]
fn format_reporter_gitlab_lists_issue_for_each_problem_when_no_file_would_change() {
    let dir = TempDir::new();
    dir.file("a.nu", FORMATTED);
    dir.file("bad.nu", "ls\nls ||");
    let output = format_in(&dir, &["--reporter", "gitlab", "a.nu", "bad.nu"]);
    assert_eq!(output.status.code(), Some(2), "{}", stderr(&output));
    let locations: Vec<_> = issues(&stdout(&output)).iter().map(location).collect();
    assert_eq!(
        locations,
        [json!(["nuten/problem", "major", "bad.nu", 2, 2])]
    );
}

#[test]
fn format_reporter_gitlab_keeps_fingerprint_of_change_while_other_lines_move() {
    let dir = TempDir::new();
    let path = dir.file("a.nu", "ls|x\n");
    let before = issues(&stdout(&format_in(&dir, &["--reporter", "gitlab", "a.nu"])));
    std::fs::write(&path, "ls\nls\nls|x\n").unwrap();
    let after = issues(&stdout(&format_in(&dir, &["--reporter", "gitlab", "a.nu"])));
    assert_eq!(after[0]["location"]["lines"]["begin"], 3);
    assert_eq!(before[0]["fingerprint"], after[0]["fingerprint"]);
}

#[test]
fn format_reporter_gitlab_names_files_from_ci_project_dir() {
    let dir = TempDir::new();
    let [a] = write_unformatted(&dir, ["d/a.nu"]);
    let bad = dir.file("d/bad.nu", "ls ||");
    let output = nuten_command()
        .args(["format", "--reporter", "gitlab"])
        .args([&a, &bad])
        .current_dir(dir.path("d"))
        .env("CI_PROJECT_DIR", &dir.0)
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2), "{}", stderr(&output));
    let paths: Vec<_> = issues(&stdout(&output))
        .iter()
        .map(|issue| issue["location"]["path"].clone())
        .collect();
    assert_eq!(paths, ["d/a.nu", "d/bad.nu"]);
}

#[test]
fn format_reporter_gitlab_tells_apart_files_alike_and_changes_of_one_file() {
    let dir = TempDir::new();
    write_unformatted(&dir, ["a.nu", "b.nu"]);
    let fingerprints = |name: &str| {
        let output = format_in(&dir, &["--reporter", "gitlab", name]);
        issues(&stdout(&output))[0]["fingerprint"].clone()
    };
    let a = fingerprints("a.nu");
    assert_ne!(fingerprints("b.nu"), a);
    dir.file("a.nu", "ls|last");
    assert_ne!(fingerprints("a.nu"), a);
}

#[test]
fn format_reporter_gitlab_lists_issue_of_stdin_under_stdin_filepath() {
    let dir = TempDir::new();
    let output = format_stdin_in(
        &dir,
        &["--reporter", "gitlab", "--stdin-filepath", "d/a.nu", "-"],
        UNFORMATTED.as_bytes(),
    );
    assert_eq!(output.status.code(), Some(1), "{}", stderr(&output));
    let locations: Vec<_> = issues(&stdout(&output)).iter().map(location).collect();
    assert_eq!(
        locations,
        [json!(["nuten/format", "minor", "d/a.nu", 1, 1])]
    );
}

#[test]
fn format_reporter_gitlab_names_files_from_current_directory_when_ci_project_dir_is_empty() {
    let dir = TempDir::new();
    write_unformatted(&dir, ["a.nu"]);
    let output = nuten_command()
        .args(["format", "--reporter", "gitlab", "a.nu"])
        .current_dir(&dir.0)
        .env("CI_PROJECT_DIR", "")
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1), "{}", stderr(&output));
    assert_eq!(issues(&stdout(&output))[0]["location"]["path"], "a.nu");
}

#[test]
fn format_reporter_gitlab_keeps_fingerprint_however_file_is_named() {
    let dir = TempDir::new();
    write_unformatted(&dir, ["d/a.nu"]);
    let fingerprint = |path: &str| {
        let output = format_in(&dir, &["--reporter", "gitlab", path]);
        issues(&stdout(&output))[0]["fingerprint"].clone()
    };
    assert_eq!(fingerprint("d"), fingerprint("./d/a.nu"));
}

#[test]
fn format_reporter_gitlab_keeps_fingerprint_wherever_project_is_checked_out() {
    let fingerprint = || {
        let dir = TempDir::new();
        write_unformatted(&dir, ["d/a.nu"]);
        let output = format_in(&dir, &["--reporter", "gitlab", "d"]);
        issues(&stdout(&output))[0]["fingerprint"].clone()
    };
    assert_eq!(fingerprint(), fingerprint());
}
