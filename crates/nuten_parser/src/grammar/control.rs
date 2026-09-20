use nuten_syntax::SyntaxKind;

use super::{Shape, call_items, item, value};
use crate::parser::Parser;

/// The `For`, `While`, `Loop`, or `Try` ahead, when nu reads the call as one.
pub(super) fn control(p: &mut Parser<'_>) -> bool {
    let Some(shape) = control_shape(&call_items(p)) else {
        return false;
    };
    p.start(shape.kind);
    p.bump();
    for at in 1..shape.len {
        if at + 1 == shape.len || (shape.has_catch && at == 1) {
            value(p, Shape::Block);
        } else if shape.kind == SyntaxKind::For && at == 1 {
            // The variable, which nu never reads as a value, braces or not.
            p.bump();
        } else {
            item(p, Shape::Any);
        }
    }
    p.finish();
    true
}

/// The shape of a `for`, a `while`, a `loop`, or a `try` among the items of a call.
/// It is built only where nu reads those items as one.
/// The last item is a body, and so is the second when `catch` follows it.
struct ControlShape {
    kind: SyntaxKind,
    len: usize,
    has_catch: bool,
}

fn control_shape(items: &[&str]) -> Option<ControlShape> {
    let braced = |at: usize| items.get(at).is_some_and(|item| item.starts_with('{'));
    let last = items.len().checked_sub(1)?;
    if last == 0 || !braced(last) {
        return None;
    }
    // `in` at `at`, then at least one item of the value before the body.
    let in_at = |at: usize| items.get(at) == Some(&"in") && last > at + 1;
    let (kind, has_catch) = match items[0] {
        // The variable may take a type after a `:` it ends with.
        "for" if in_at(2) || (items[1].ends_with(':') && in_at(3)) => (SyntaxKind::For, false),
        "while" if last >= 2 => (SyntaxKind::While, false),
        "loop" if last == 1 => (SyntaxKind::Loop, false),
        "try" if last == 1 => (SyntaxKind::Try, false),
        "try" if last == 3 && braced(1) && items[2] == "catch" => (SyntaxKind::Try, true),
        _ => return None,
    };
    Some(ControlShape {
        kind,
        len: items.len(),
        has_catch,
    })
}
