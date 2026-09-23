use nuten_syntax::SyntaxKind;

use super::{at_call_item, call_items, is_flag};
use crate::parser::Parser;

/// The `module` call ahead, where nu reads no body in it, with its items as written.
/// Nu takes nothing but a string for the name, and reads nothing past the name.
/// Any more is an error, unless a help flag makes the call do nothing else.
pub(super) fn module(p: &mut Parser<'_>) -> bool {
    if takes_body(&call_items(p)) {
        return false;
    }
    p.start(SyntaxKind::Call);
    while at_call_item(p) {
        p.bump();
    }
    p.finish();
    true
}

/// Whether nu reads a body in a `module` call of `items`: `module`, the name, and `{ }`.
fn takes_body(items: &[&str]) -> bool {
    matches!(items, [_, name, body] if !is_flag(name) && body.starts_with('{'))
}
