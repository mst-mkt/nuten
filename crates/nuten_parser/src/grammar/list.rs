use nuten_syntax::SyntaxKind;

use super::{
    Shape,
    spread::{Target, is_spread, spread},
    value,
};
use crate::parser::Parser;

/// A `;` may make it a table, or else a word.
/// A lexing error keeps it a word too, since nu lexed the word whole without one.
pub(super) fn is_list(inner: &Parser<'_>) -> bool {
    !inner
        .remaining_kinds()
        .any(|kind| matches!(kind, SyntaxKind::Semicolon | SyntaxKind::Error))
}

/// Unlike `item`, reports no shell operator.
pub(super) fn list_items(p: &mut Parser<'_>) {
    while let Some(text) = p.peek_text() {
        if is_spread(text, Target::List) {
            spread(p, Target::List);
        } else {
            value(p, Shape::Any);
        }
    }
}

/// The items of a list whose items nu reads as another type than any value, each as `shape`.
/// A spread stays a word, which reads the same as its value would.
pub(super) fn items_as(p: &mut Parser<'_>, shape: Shape) {
    while p.peek().is_some() {
        value(p, shape);
    }
}
