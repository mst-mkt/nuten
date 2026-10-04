#[cfg(target_os = "linux")]
use std::fs;
use std::{ffi::OsStr, io};

use crate::support::{nuten, nuten_command, stderr, stdout};

#[test]
fn completions_prints_script_whose_request_names_given_shell() {
    for shell in ["bash", "elvish", "fish", "nu", "powershell", "zsh"] {
        let output = nuten(&[OsStr::new("completions"), OsStr::new(shell)]);
        assert_eq!(
            output.status.code(),
            Some(0),
            "{shell}: {}",
            stderr(&output)
        );
        assert!(output.stderr.is_empty(), "{shell}");
        let stdout = stdout(&output);
        let request = format!("__complete_word__ --shell {shell}");
        // The header comment names the request too, so it alone would pass.
        let has_request = stdout
            .lines()
            .filter(|line| !line.trim_start().starts_with('#'))
            .any(|line| line.contains(&request));
        assert!(has_request, "{shell}: {stdout}");
    }
}

#[test]
fn completions_rejects_unknown_shell() {
    let output = nuten(&[OsStr::new("completions"), OsStr::new("sh")]);
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
}

#[test]
fn completions_succeeds_when_reader_closes_first() {
    let (reader, writer) = io::pipe().unwrap();
    drop(reader);
    let output = nuten_command()
        .args(["completions", "nu"])
        .stdout(writer)
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    assert!(output.stderr.is_empty());
}

#[cfg(target_os = "linux")]
#[test]
fn completions_reports_write_error() {
    let full = fs::File::options().write(true).open("/dev/full").unwrap();
    let output = nuten_command()
        .args(["completions", "nu"])
        .stdout(full)
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    let stderr = stderr(&output);
    assert!(stderr.starts_with("stdout: "), "{stderr}");
}

/// A line starting with U+0001 asks the script to complete something else, such as files.
fn completion_answers(line: &str) -> Vec<String> {
    let output = nuten(&["__complete_word__", "--shell", "nu", "--line", line].map(OsStr::new));
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    stdout(&output).lines().map(str::to_string).collect()
}

/// Sorted, since the order is up to `usage`, not nuten.
fn completion_candidates(line: &str) -> Vec<String> {
    let mut candidates: Vec<_> = completion_answers(line)
        .iter()
        .filter(|answer| !answer.starts_with('\u{1}'))
        .map(|answer| answer.split('\t').next().unwrap().to_string())
        .collect();
    candidates.sort_unstable();
    candidates
}

/// Sorted, since their order means nothing to a shell.
fn completion_extensions(line: &str) -> Vec<String> {
    let answers = completion_answers(line);
    let mut extensions: Vec<_> = answers
        .iter()
        .find_map(|answer| answer.strip_prefix("\u{1}extensions\t"))
        .unwrap_or_else(|| panic!("no extensions for {line:?}: {answers:?}"))
        .split('\t')
        .map(str::to_string)
        .collect();
    extensions.sort_unstable();
    extensions
}

#[test]
fn complete_word_offers_subcommands() {
    assert_eq!(
        completion_candidates("nuten "),
        ["completions", "format", "lsp", "schema"]
    );
}

#[test]
fn complete_word_offers_subcommand_flags_that_start_with_word() {
    assert_eq!(
        completion_candidates("nuten format --c"),
        ["--check", "--config"]
    );
}

#[test]
fn complete_word_offers_shells_for_completions() {
    assert_eq!(
        completion_candidates("nuten completions "),
        ["bash", "elvish", "fish", "nu", "powershell", "zsh"]
    );
}

#[test]
fn complete_word_asks_for_nu_and_nuon_files_for_paths() {
    for line in [
        "nuten format ",
        "nuten format a.nu ",
        "nuten format --config a.nuon ",
        "nuten format -c a.nuon ",
        "nuten format -- ",
        "nuten format -- -c ",
    ] {
        assert_eq!(completion_extensions(line), ["nu", "nuon"], "{line}");
    }
}

#[test]
fn complete_word_asks_for_nu_and_nuon_files_for_stdin_filepath() {
    for line in [
        "nuten format --stdin-filepath ",
        "nuten format --stdin-filepath=",
    ] {
        assert_eq!(completion_extensions(line), ["nu", "nuon"], "{line}");
    }
}

#[test]
fn complete_word_asks_for_config_files_for_config() {
    let mut expected = nuten_config::Format::ALL.map(nuten_config::Format::extension);
    expected.sort_unstable();
    for line in [
        "nuten format --config ",
        "nuten format -c ",
        "nuten format --config=",
    ] {
        assert_eq!(completion_extensions(line), expected, "{line}");
    }
}
