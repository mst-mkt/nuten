use nuten_syntax::SyntaxKind;

use super::{CURLY, Shape, is_item, item, shell_operator, value};
use crate::{lexer::Mode, parser::Parser};

/// How many items a `match` takes when nu reads `items` as one: `match`,
/// the subject, and the arms in braces last.
pub(super) fn match_shape(items: &[&str]) -> Option<usize> {
    let last = items.len().checked_sub(1)?;
    (items[0] == "match" && last >= 2 && items[last].starts_with('{')).then_some(items.len())
}

/// The `Match` of the `len` items ahead: `match`, the subject, which nu
/// reads as any value, and the arms, a `MatchArms` where nu reads arms and
/// otherwise the word as it is, which nu rejects.
pub(super) fn match_(p: &mut Parser<'_>, len: usize) {
    p.start(SyntaxKind::Match);
    p.bump();
    for _ in 1..len - 1 {
        item(p, Shape::Any);
    }
    if let Some(bracketed) = p.bracketed(&CURLY, Mode::match_arms())
        && is_arms(bracketed.inner())
    {
        p.bump_bracketed(SyntaxKind::MatchArms, bracketed, arms);
    } else {
        p.bump();
    }
    p.finish();
}

/// Whether the `{ }` of a `match` holds arms as nu reads them, near enough:
/// each a pattern of items, a guard among them, with a `|` between two of
/// them, then `=>` and one item, with any number of `,` between arms. Nu
/// takes a `,` anywhere and an operator as a pattern too, and rejects some
/// runs of items; it errors on them all the same before and after.
fn is_arms(inner: &Parser<'_>) -> bool {
    let takes = |kind: SyntaxKind, text: &str| {
        is_item(kind) && kind != SyntaxKind::Error && text != "=>" && shell_operator(text).is_none()
    };
    let mut tokens = inner
        .remaining_tokens()
        .filter(|(token, _)| !token.kind.is_trivia())
        .peekable();
    loop {
        while tokens
            .next_if(|(token, _)| token.kind == SyntaxKind::Comma)
            .is_some()
        {}
        if tokens.peek().is_none() {
            return true;
        }
        // The pattern, up to `=>`.
        let mut after_item = false;
        loop {
            let Some((token, text)) = tokens.next() else {
                return false;
            };
            match token.kind {
                SyntaxKind::Pipe if after_item => after_item = false,
                SyntaxKind::Word if text == "=>" && after_item => break,
                kind if takes(kind, text) => after_item = true,
                _ => return false,
            }
        }
        // What the arm does.
        if !tokens
            .next()
            .is_some_and(|(token, text)| takes(token.kind, text))
        {
            return false;
        }
    }
}

/// The arms, with the `,` between them.
fn arms(p: &mut Parser<'_>) {
    while let Some(kind) = p.peek() {
        if matches!(kind, SyntaxKind::Comma | SyntaxKind::Newline) {
            p.bump();
            continue;
        }
        p.start(SyntaxKind::MatchArm);
        while p.peek_text() != Some("=>") {
            p.bump();
        }
        p.bump_as(SyntaxKind::FatArrow);
        while p.peek() == Some(SyntaxKind::Newline) {
            p.bump();
        }
        value(p, Shape::Block);
        p.finish();
    }
}
