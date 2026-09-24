//! Inputs shared by tests, and a way to ask nu.

use std::process::Command;

/// What nu prints for `pipeline`, line by line.
/// `None` when nu cannot be run, so that a machine without it still runs every other test.
/// Nu failing means the pipeline no longer asks what it meant to, which must not pass.
pub(crate) fn asked(pipeline: &str) -> Option<Vec<String>> {
    let output = Command::new("nu")
        .args(["-n", "-c", pipeline])
        .output()
        .ok()?;
    assert!(
        output.status.success(),
        "{pipeline}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    Some(
        String::from_utf8_lossy(&output.stdout)
            .lines()
            .map(str::to_owned)
            .collect(),
    )
}

/// A sample of each byte class the lexer tells apart.
/// `à` (`C3 A0`) ends in a byte `starts_comment_after` counts as whitespace.
const ALPHABET: &[&str] = &[
    "e", "<", ">", "=", "r", "$", "'", "\"", "`", "\\", "(", ")", "[", "]", "{", "}", "#", "|",
    ";", ",", ":", " ", "\n", "\r", "à",
];

pub(crate) fn short_inputs() -> impl Iterator<Item = String> {
    (0..=4).flat_map(inputs_of_len)
}

fn inputs_of_len(len: usize) -> Vec<String> {
    match len {
        0 => vec![String::new()],
        _ => inputs_of_len(len - 1)
            .iter()
            .flat_map(|src| ALPHABET.iter().map(move |ch| format!("{src}{ch}")))
            .collect(),
    }
}
