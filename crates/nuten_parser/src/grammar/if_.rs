use nuten_syntax::SyntaxKind;

use super::{Shape, call_items, command::condition, item, value};
use crate::parser::Parser;

/// The `If` ahead, when nu reads the call as one.
pub(super) fn if_(p: &mut Parser<'_>) -> bool {
    let Some(chain) = if_chain(&call_items(p)) else {
        return false;
    };
    p.start(SyntaxKind::If);
    let mut at = 0;
    for link in &chain {
        p.bump();
        condition(p, link.body - at - 1);
        value(p, Shape::Block);
        at = link.body + 1;
        if link.has_else {
            p.bump();
            at += 1;
        }
    }
    // The one item after the last `else`.
    if chain.last().is_some_and(|link| link.has_else) {
        item(p, Shape::Block);
    }
    p.finish();
    true
}

/// One `if` of a chain: where its body is among the items of the chain.
/// It carries whether `else` follows that body too.
struct IfLink {
    body: usize,
    has_else: bool,
}

/// The `if`s that `items` make when nu reads them as one, in order.
/// Each is `if`, a condition, and a body in braces.
/// The body is the item right before the first `else`, or the last item.
/// After an `else` comes one item, or the next `if`.
fn if_chain(items: &[&str]) -> Option<Vec<IfLink>> {
    let mut chain = Vec::new();
    let mut start = 0;
    loop {
        let rest = &items[start..];
        if rest.first() != Some(&"if") {
            return None;
        }
        let otherwise = rest.iter().position(|&item| item == "else");
        let body = match otherwise {
            Some(otherwise) => otherwise.checked_sub(1)?,
            None => rest.len() - 1,
        };
        if body < 2 || !rest[body].starts_with('{') {
            return None;
        }
        chain.push(IfLink {
            body: start + body,
            has_else: otherwise.is_some(),
        });
        // Items after `else`.
        match otherwise.map(|otherwise| rest.len() - otherwise - 1) {
            None | Some(1) => return Some(chain),
            Some(0) => return None,
            Some(_) => start += body + 2,
        }
    }
}
