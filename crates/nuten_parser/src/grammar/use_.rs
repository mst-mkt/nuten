use nuten_syntax::SyntaxKind;

use super::{
    Shape, at_call_item, call_items, item, keyword_words, leading_flags,
    spread::{Target, is_spread, spread},
};
use crate::parser::Parser;

pub(super) fn use_(p: &mut Parser<'_>) -> bool {
    let items = call_items(p);
    let words = keyword_words(p);
    if items.len() <= words {
        return false;
    }
    p.start(SyntaxKind::Use);
    p.bump_n(words + leading_flags(&items[words..]));
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
