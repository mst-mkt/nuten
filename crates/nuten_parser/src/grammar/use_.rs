use nuten_syntax::SyntaxKind;

use super::{
    Shape, at_call_item, call_items, item, leading_flags,
    spread::{Target, is_spread, spread},
};
use crate::parser::Parser;

/// The `Use` ahead, when the call takes an item past `use`.
pub(super) fn use_(p: &mut Parser<'_>) -> bool {
    let items = call_items(p);
    if items.len() < 2 {
        return false;
    }
    p.start(SyntaxKind::Use);
    for _ in 0..=leading_flags(&items[1..]) {
        p.bump();
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
