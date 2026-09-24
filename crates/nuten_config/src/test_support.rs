//! Helpers the tests of every module share.

use crate::{Format, load};

/// Each error `load` gives, as its message and the text it is at.
pub(crate) fn errors(text: &str, format: Format) -> Vec<(String, &str)> {
    load(text, format)
        .unwrap_err()
        .into_iter()
        .map(|error| (error.kind.to_string(), &text[error.range]))
        .collect()
}
