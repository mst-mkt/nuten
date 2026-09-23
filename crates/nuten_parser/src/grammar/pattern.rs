use nuten_syntax::SyntaxKind;

use super::{SQUARE, Shape, value};
use crate::{
    lexer::{Mode, Token},
    parser::Parser,
};

/// The pattern ahead as nu reads it, which nu tells by its first byte.
/// A variable or a record pattern stays as it is.
/// Anything else but a list pattern is a value, `_` among them.
pub(super) fn pattern(p: &mut Parser<'_>) {
    match p.peek_text().map(str::as_bytes) {
        Some([b'$' | b'{', ..]) => p.bump(),
        Some([b'[', ..]) => list_pattern(p),
        _ => value(p, Shape::Any),
    }
}

/// The tokens inside the brackets of a pattern, the `,` and trivia among them aside.
fn pattern_tokens<'a>(inner: &Parser<'a>) -> Vec<(Token, &'a str)> {
    inner
        .remaining_tokens()
        .filter(|(token, _)| !token.kind.is_trivia() && token.kind != SyntaxKind::Comma)
        .collect()
}

/// Whether a token of `kind` is one pattern, or a key, where it stands.
/// Unlike for `is_item`, an `Error` is none, since nuten reads no pattern of its text.
fn is_pattern_item(kind: SyntaxKind) -> bool {
    kind.is_item() && kind != SyntaxKind::Error
}

/// The `[ ]` ahead as a `ListPattern`, or as a word where nu reads less than it holds.
/// Nu splits the patterns at a `|`, and reads none past a `;` or a redirection.
/// It reads none past a rest either.
/// It takes `=` as a string, but nuten lexes it as an operator.
fn list_pattern(p: &mut Parser<'_>) {
    match p.bracketed(&SQUARE, Mode::List) {
        Some(bracketed) if reads_whole(bracketed.inner()) => {
            p.bump_bracketed(SyntaxKind::ListPattern, bracketed, list_pattern_items);
        },
        _ => p.bump(),
    }
}

/// Whether the inside of `[ ]` holds nothing but patterns, a rest only last.
fn reads_whole(inner: &Parser<'_>) -> bool {
    let tokens = pattern_tokens(inner);
    let rest = tokens.iter().position(|&(_, text)| is_rest(text));
    tokens.iter().all(|(token, _)| is_pattern_item(token.kind))
        && rest.is_none_or(|rest| rest + 1 == tokens.len())
}

/// Whether nu reads `text` as the rest of a list pattern: `..` alone, or before a `$`.
fn is_rest(text: &str) -> bool {
    text == ".." || text.starts_with("..$")
}

/// The patterns of a list pattern, and the rest.
fn list_pattern_items(p: &mut Parser<'_>) {
    while let Some((kind, text)) = p.peek().zip(p.peek_text()) {
        if matches!(kind, SyntaxKind::Comma | SyntaxKind::Newline) || is_rest(text) {
            p.bump();
        } else {
            pattern(p);
        }
    }
}
