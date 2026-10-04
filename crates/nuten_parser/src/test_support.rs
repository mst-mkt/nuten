use std::{env, process::Command};

use rowan::TextRange;

use crate::{ParseErrorKind, parse};

pub(crate) fn range(start: u32, end: u32) -> TextRange {
    TextRange::new(start.into(), end.into())
}

pub(crate) fn errors(src: &str) -> Vec<ParseErrorKind> {
    parse(src).errors().iter().map(|error| error.kind).collect()
}

/// Nu failing means the pipeline no longer asks what it meant to, which must not pass.
pub(crate) fn asked(pipeline: &str) -> Option<Vec<String>> {
    let output = match Command::new("nu").args(["-n", "-c", pipeline]).output() {
        Ok(output) => output,
        Err(_) if env::var_os("NUTEN_TEST_SKIP_NU").is_some() => return None,
        Err(error) => panic!("nu: {error}; set NUTEN_TEST_SKIP_NU to skip the tests that need nu"),
    };
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
