use nuten_syntax::SyntaxKind;

use super::{
    CURLY, at_call_item,
    block::{holds_more_than_trivia, params_start},
    call_items, file, is_flag, keyword_words,
};
use crate::{lexer::Mode, parser::Parser};

/// The `Module` ahead, or else the `module` call ahead with its items as written.
/// Nu takes nothing but a string for the name, so it stays as written either way.
pub(super) fn module(p: &mut Parser<'_>) -> bool {
    let words = keyword_words(p);
    if call_items(p).get(words..).is_some_and(takes_body) {
        module_with_body(p, words);
    } else {
        module_without_body(p);
    }
    true
}

/// The `Module` ahead: the `words` of its keyword, the name, and the body.
fn module_with_body(p: &mut Parser<'_>, words: usize) {
    p.start(SyntaxKind::Module);
    for _ in 0..=words {
        p.bump();
    }
    body(p);
    p.finish();
}

/// The `module` call ahead, where nu reads nothing past the name.
/// Any more is an error, unless a help flag makes the call do nothing else.
fn module_without_body(p: &mut Parser<'_>) {
    p.start(SyntaxKind::Call);
    while at_call_item(p) {
        p.bump();
    }
    p.finish();
}

/// Whether nu reads a body where `items` follow the keyword: the name and `{ }`.
fn takes_body(items: &[&str]) -> bool {
    matches!(items, [name, body] if !is_flag(name) && body.starts_with('{'))
}

/// Nu reads what the braces hold as statements, whatever they are.
/// An empty body stays a `Word`, as any empty `{ }` does.
/// So does one that starts with a `|`, which a block lays out apart from its brace.
/// Nu reads no parameters there, so that would only add a line.
fn body(p: &mut Parser<'_>) {
    match p.bracketed(&CURLY, Mode::Block) {
        Some(bracketed)
            if holds_more_than_trivia(bracketed.inner())
                && params_start(bracketed.inner()).is_none() =>
        {
            p.bump_bracketed_or_word(SyntaxKind::Block, bracketed, file);
        },
        _ => p.bump(),
    }
}
