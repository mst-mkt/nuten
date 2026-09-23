use super::{Shape, value};
use crate::parser::Parser;

/// The pattern ahead as nu reads it, which nu tells by its first byte.
/// A variable, a record, or a list pattern stays as it is.
/// Anything else is a value, `_` among them.
pub(super) fn pattern(p: &mut Parser<'_>) {
    match p.peek_text().map(str::as_bytes) {
        Some([b'$' | b'{' | b'[', ..]) => p.bump(),
        _ => value(p, Shape::Any),
    }
}
