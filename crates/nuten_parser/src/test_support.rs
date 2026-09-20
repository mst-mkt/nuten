//! Inputs shared by tests.

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
