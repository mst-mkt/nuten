use std::iter;

use nuten_syntax::SyntaxKind;

use super::{
    CURLY, Shape, call_items, command::expression, item, pattern::pattern, shell_operator, value,
};
use crate::{lexer::Mode, parser::Parser};

pub(super) fn match_(p: &mut Parser<'_>) -> bool {
    let Some(len) = match_shape(&call_items(p)) else {
        return false;
    };
    p.start(SyntaxKind::Match);
    p.bump();
    for _ in 1..len - 1 {
        item(p, Shape::Any);
    }
    if let Some(bracketed) = p.bracketed(&CURLY, Mode::MatchArms)
        && is_arms(bracketed.inner())
    {
        p.bump_bracketed(SyntaxKind::MatchArms, bracketed, arms);
    } else {
        p.bump();
    }
    p.finish();
    true
}

fn match_shape(items: &[&str]) -> Option<usize> {
    let last = items.len().checked_sub(1)?;
    (last >= 2 && items[last].starts_with('{')).then_some(items.len())
}

/// Nu takes a `,` anywhere and an operator as a pattern too, and rejects some runs of items.
/// It errors on them all the same before and after.
fn is_arms(inner: &Parser<'_>) -> bool {
    let takes = |kind: SyntaxKind, text: &str| {
        kind.is_item()
            && kind != SyntaxKind::Error
            && text != "=>"
            && shell_operator(text).is_none()
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
        if !tokens
            .next()
            .is_some_and(|(token, text)| takes(token.kind, text))
        {
            return false;
        }
    }
}

fn arms(p: &mut Parser<'_>) {
    while let Some(kind) = p.peek() {
        if kind == SyntaxKind::Comma {
            p.bump();
        } else {
            arm(p);
        }
    }
}

enum ArmHead {
    Patterns,
    Guarded,
    Rejected,
}

fn arm(p: &mut Parser<'_>) {
    p.start(SyntaxKind::MatchArm);
    match arm_head(&arm_tokens(p)) {
        ArmHead::Patterns => patterns(p),
        ArmHead::Guarded => {
            patterns(p);
            p.bump();
            guard(p);
        },
        ArmHead::Rejected => {},
    }
    while p.peek_text() != Some("=>") {
        p.bump();
    }
    p.bump_as(SyntaxKind::FatArrow);
    value(p, Shape::Block);
    p.finish();
}

fn patterns(p: &mut Parser<'_>) {
    pattern(p);
    while p.peek() == Some(SyntaxKind::Pipe) {
        p.bump();
        pattern(p);
    }
}

/// A guard holding a `|` stays as it is, since nu rejects that `|` as an operator.
fn guard(p: &mut Parser<'_>) {
    let tokens = arm_tokens(p);
    if tokens.iter().any(|&(kind, _)| kind == SyntaxKind::Pipe) {
        return;
    }
    let arrow = p
        .remaining_tokens()
        .find(|&(_, text)| text == "=>")
        .map(|(token, _)| token.range.start())
        .expect("an arm has `=>`");
    p.bump_bounded(arrow, expression);
}

fn arm_tokens<'a>(p: &Parser<'a>) -> Vec<(SyntaxKind, &'a str)> {
    p.remaining_tokens()
        .filter(|(token, _)| !p.is_skipped(token.kind))
        .map(|(token, text)| (token.kind, text))
        .take_while(|&(_, text)| text != "=>")
        .collect()
}

/// `is_arms` has seen that each pattern is an item.
fn arm_head(tokens: &[(SyntaxKind, &str)]) -> ArmHead {
    let after_patterns = iter::successors(tokens.get(1..), |rest| match rest {
        [(SyntaxKind::Pipe, _), _, rest @ ..] => Some(rest),
        _ => None,
    })
    .last();
    match after_patterns {
        Some([]) => ArmHead::Patterns,
        Some([(_, "if"), _, ..]) => ArmHead::Guarded,
        _ => ArmHead::Rejected,
    }
}
