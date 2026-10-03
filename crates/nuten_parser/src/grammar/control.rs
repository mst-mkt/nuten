use nuten_syntax::SyntaxKind;

use super::{Shape, call_items, command::condition, item, value, var_type::var_type};
use crate::parser::Parser;

pub(super) fn control(p: &mut Parser<'_>) -> bool {
    let items = call_items(p);
    let Some(shape) = control_shape(&items) else {
        return false;
    };
    p.start(shape.kind);
    p.bump();
    if shape.kind == SyntaxKind::For {
        for_variable(p);
    }
    if shape.kind == SyntaxKind::While {
        condition(p, &items[1..shape.values_at]);
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

/// Nu never reads the variable of a `for` or its type as a value, braces or not.
fn for_variable(p: &mut Parser<'_>) {
    if !var_type(p, "for") {
        p.bump();
    }
    while p.peek_text() != Some("in") {
        p.bump();
    }
}

struct ControlShape {
    kind: SyntaxKind,
    len: usize,
    has_catch: bool,
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

/// Nu ends the variable at the first `in` from the variable on.
/// Only a name ending in `:` takes the items of its type before the `in`.
/// At least one item of the value comes between the `in` and the body.
fn for_in(items: &[&str]) -> Option<usize> {
    let at = items.iter().skip(1).position(|&item| item == "in")? + 1;
    let variable = at == 2 || (at > 2 && items[1].ends_with(':'));
    (variable && items.len() > at + 2).then_some(at)
}
