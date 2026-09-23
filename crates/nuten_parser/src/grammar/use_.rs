use nuten_syntax::SyntaxKind;

use super::{
    Shape, at_call_item, call_items, is_flag, item,
    spread::{Target, is_spread, spread},
};
use crate::parser::Parser;

/// The `Use` ahead, when the call takes an item past `use`.
/// Nu reads no flag past a `--`.
pub(super) fn use_(p: &mut Parser<'_>) -> bool {
    if call_items(p).len() < 2 {
        return false;
    }
    p.start(SyntaxKind::Use);
    p.bump();
    while at_call_item(p)
        && let Some(flag) = p.peek_text().filter(|text| is_flag(text))
    {
        p.bump();
        if flag == "--" {
            break;
        }
    }
    if at_call_item(p) {
        item(p, Shape::KnownArgument);
    }
    while at_call_item(p) {
        member(p);
    }
    p.finish();
    true
}

fn member(p: &mut Parser<'_>) {
    if p.peek_text()
        .is_some_and(|text| is_spread(text, Target::List))
    {
        spread(p, Target::List);
    } else {
        item(p, Shape::Any);
    }
}
