use crate::{Format, load};

pub(crate) fn errors(text: &str, format: Format) -> Vec<(String, &str)> {
    load(text, format)
        .unwrap_err()
        .into_iter()
        .map(|error| (error.kind.to_string(), &text[error.range]))
        .collect()
}
