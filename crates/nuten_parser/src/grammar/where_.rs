use nuten_syntax::SyntaxKind;

use super::{
    CURLY, block::block, call_items, command::expression, keyword::keyword_as_term, leading_flags,
};
use crate::{lexer::Mode, parser::Parser};

/// The `Where` ahead, when the call takes a row condition past its flags.
pub(super) fn where_(p: &mut Parser<'_>) -> bool {
    let items = call_items(p);
    let flags = leading_flags(&items[1..]);
    let condition = &items[1 + flags..];
    if condition.is_empty() {
        return false;
    }
    p.start(SyntaxKind::Where);
    for _ in 0..=flags {
        p.bump();
    }
    row_condition(p, condition);
    p.finish();
    true
}

/// The row condition ahead, whose items are `condition`.
/// Nu first tries a closure over the whole of it, which only a lone `{ }` makes.
/// Failing that, it reads the row condition as an expression.
/// An `if` or a `match` first heads a call over it instead.
fn row_condition(p: &mut Parser<'_>, condition: &[&str]) {
    if let [_] = condition
        && let Some(bracketed) = p.bracketed(&CURLY, Mode::Block)
    {
        block(p, bracketed);
    } else if !keyword_as_term(p) {
        expression(p);
    }
}
