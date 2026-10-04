use nuten_syntax::SyntaxKind;

use super::{CURLY, block::block, call_items, command::expression, leading_flags};
use crate::{lexer::Mode, parser::Parser};

pub(super) fn where_(p: &mut Parser<'_>) -> bool {
    let items = call_items(p);
    let flags = leading_flags(&items[1..]);
    let condition = &items[1 + flags..];
    if condition.is_empty() {
        return false;
    }
    p.start(SyntaxKind::Where);
    p.bump_n(flags + 1);
    row_condition(p, condition);
    p.finish();
    true
}

/// Nu first tries a closure over the whole row condition, which only a lone `{ }` makes.
/// Failing that, it reads the row condition as an expression.
fn row_condition(p: &mut Parser<'_>, condition: &[&str]) {
    if let [_] = condition
        && let Some(bracketed) = p.bracketed(&CURLY, Mode::Block)
    {
        block(p, bracketed);
    } else {
        expression(p);
    }
}
