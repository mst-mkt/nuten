use nuten_syntax::SyntaxKind;

use super::{
    Shape, call_items,
    command::{command, condition},
    item, value,
};
use crate::parser::Parser;

/// The `If` ahead, when nu reads the call as one.
pub(super) fn if_(p: &mut Parser<'_>) -> bool {
    let items = call_items(p);
    let Some(chain) = if_chain(&items) else {
        return false;
    };
    p.start(SyntaxKind::If);
    let mut at = 0;
    for link in &chain {
        p.bump();
        condition(p, &items[at + 1..link.body]);
        value(p, Shape::Block);
        at = link.body + 1;
        if link.has_else {
            p.bump();
            at += 1;
        }
    }
    if chain.last().is_some_and(|link| link.has_else) {
        else_argument(p, items.len() - at);
    }
    p.finish();
    true
}

/// What the last `else` takes, its `len` items: one as a block, or all of them as one expression.
/// Nu reads more than one as what heads a statement, up to the last item of the `if` call.
/// A redirection after them belongs to the whole `if`, so the expression takes none.
fn else_argument(p: &mut Parser<'_>, len: usize) {
    if len == 1 {
        item(p, Shape::Block);
        return;
    }
    let end = p
        .nth_range(len - 1)
        .expect("an item after `else` ahead")
        .end();
    // Nu reads no attribute there, and the items hold no assignment operator.
    p.bump_bounded(end, |p| {
        command(p, false);
    });
}

/// One `if` of a chain: where its body is among the items of the chain.
/// It carries whether `else` follows that body too.
struct IfLink {
    body: usize,
    has_else: bool,
}

/// The `if`s that `items` make when nu reads them as one, in order.
/// After an `else` comes one item, or more items as one expression.
/// Nu reads that expression as an `if` call where it starts with `if`, which is one more link here.
/// Nu reads a first item it takes as a block alone, and the rest as more items of the `if` call.
/// So that makes no `If`.
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
        let Some(otherwise) = otherwise else {
            return Some(chain);
        };
        match &rest[otherwise + 1..] {
            [] => return None,
            [_] => return Some(chain),
            ["if", ..] => start += otherwise + 1,
            [first, ..] if taken_as_block(first) => return None,
            _ => return Some(chain),
        }
    }
}

/// Whether nu takes `item` where it takes a block, whatever the item holds.
/// It reads a `{ }`, a `$`, a `( )`, and a raw string before it looks at the shape.
fn taken_as_block(item: &str) -> bool {
    item.starts_with(['{', '$', '(']) || item.starts_with("r#")
}
