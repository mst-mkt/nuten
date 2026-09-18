use nuten_syntax::SyntaxKind;

use super::{Shape, value};
use crate::parser::Parser;

/// A `;` may make it a table, or else a word. A lexing error keeps it a word
/// too, since nothing inside may be reported.
pub(super) fn is_list(inner: &Parser<'_>) -> bool {
    !inner
        .remaining_kinds()
        .any(|kind| matches!(kind, SyntaxKind::Semicolon | SyntaxKind::Error))
}

/// Unlike `item`, reports no error: nu may read the word as a string.
pub(super) fn list_items(p: &mut Parser<'_>) {
    while p.peek().is_some() {
        value(p, Shape::Any);
    }
}
