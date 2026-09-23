use nuten_syntax::SyntaxKind;

use super::{Shape, call_items, command::condition, item, value, var_type::var_type};
use crate::parser::Parser;

/// The `For`, `While`, `Loop`, or `Try` ahead, when nu reads the call as one.
pub(super) fn control(p: &mut Parser<'_>) -> bool {
    let Some(shape) = control_shape(&call_items(p)) else {
        return false;
    };
    p.start(shape.kind);
    p.bump();
    if shape.kind == SyntaxKind::For {
        for_variable(p);
    }
    if shape.kind == SyntaxKind::While {
        condition(p, shape.values_at - 1);
    }
    for at in shape.values_at..shape.len {
        if at + 1 == shape.len || (shape.has_catch && at == 1) {
            value(p, Shape::Block);
        } else {
            item(p, Shape::Any);
        }
    }
    p.finish();
    true
}

/// The variable of a `for` and any type after it, up to the `in`.
/// Nu never reads them as a value, braces or not.
fn for_variable(p: &mut Parser<'_>) {
    if !var_type(p, "for") {
        p.bump();
    }
    while p.peek_text() != Some("in") {
        p.bump();
    }
}

/// The shape of a `for`, a `while`, a `loop`, or a `try` among the items of a call.
/// It is built only where nu reads those items as one.
/// The last item is a body, and so is the second when `catch` follows it.
struct ControlShape {
    kind: SyntaxKind,
    len: usize,
    has_catch: bool,
    /// Where the items nu reads as values start.
    /// That is at the `in` of a `for`, at the body of a `while`, and after the keyword otherwise.
    /// The condition of a `while` comes before them, which nu reads as an expression.
    values_at: usize,
}

fn control_shape(items: &[&str]) -> Option<ControlShape> {
    let braced = |at: usize| items.get(at).is_some_and(|item| item.starts_with('{'));
    let last = items.len().checked_sub(1)?;
    if last == 0 || !braced(last) {
        return None;
    }
    let (kind, has_catch, values_at) = match items[0] {
        "for" => (SyntaxKind::For, false, for_in(items)?),
        "while" if last >= 2 => (SyntaxKind::While, false, last),
        "loop" if last == 1 => (SyntaxKind::Loop, false, 1),
        "try" if last == 1 => (SyntaxKind::Try, false, 1),
        "try" if last == 3 && braced(1) && items[2] == "catch" => (SyntaxKind::Try, true, 1),
        _ => return None,
    };
    Some(ControlShape {
        kind,
        len: items.len(),
        has_catch,
        values_at,
    })
}

/// Where the `in` of a `for` is among `items`.
/// Nu ends the variable at the first `in` from the variable on.
/// Only a name ending in `:` takes the items of its type before the `in`.
/// At least one item of the value comes between the `in` and the body.
fn for_in(items: &[&str]) -> Option<usize> {
    let at = items.iter().skip(1).position(|&item| item == "in")? + 1;
    let variable = at == 2 || (at > 2 && items[1].ends_with(':'));
    (variable && items.len() > at + 2).then_some(at)
}
