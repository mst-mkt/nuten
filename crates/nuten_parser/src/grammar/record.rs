use nuten_syntax::SyntaxKind;

use super::{
    Shape,
    spread::{Target, is_spread, spread},
    value,
};
use crate::{
    lexer::{Mode, Token},
    parser::{Bracketed, Parser},
};

/// The `{ }` ahead as a `Record`, or as a word.
/// It stays a word when its tokens cannot make nu's split.
/// It stays one too when they show another `RecordSign` without the `,` among them.
/// A `,` is whitespace to nu inside a record, so the formatter drops every one.
/// Nu would then read the `{ }` by the other sign.
/// `sign` is what `record_sign` found in brace mode.
/// `bracketed` is what the `{ }` holds in record mode.
pub(super) fn record<'a>(p: &mut Parser<'a>, bracketed: Bracketed<'a>, sign: RecordSign) {
    let without_commas = bracketed
        .inner()
        .remaining_tokens()
        .filter(|(token, _)| token.kind != SyntaxKind::Comma);
    if record_sign(without_commas) != Some(sign) {
        p.bump();
        return;
    }
    match split_record(bracketed.inner()) {
        Some(items) => p.bump_bracketed(SyntaxKind::Record, bracketed, |p| {
            record_items(p, &items);
        }),
        None => p.bump(),
    }
}

/// What makes nu read `{ }` as a record where it takes any value.
/// Nu reads a `:` second as a record whatever it takes.
/// It reads a spread first as one only where it takes no closure, block, or match arms.
/// It reads nothing but trivia as one only where it takes no closure, block, or match arms either.
/// So which sign shows must not depend on a `,`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum RecordSign {
    ColonSecond,
    SpreadFirst,
    Empty,
}

/// The `RecordSign` that the first two of `tokens` show, trivia aside.
/// `tokens` are the inside of `{ }`.
/// `Empty` when there are none.
/// `None` when a pipe comes first, or when what comes first shows no sign.
/// Nu looks with `,` as no whitespace, so brace mode tells what it sees.
/// Record mode without its `,` tells what it would see in the output.
pub(super) fn record_sign<'a>(
    tokens: impl Iterator<Item = (Token, &'a str)>,
) -> Option<RecordSign> {
    let mut tokens = tokens.filter(|(token, _)| !token.kind.is_trivia());
    match (tokens.next(), tokens.next()) {
        (Some((first, text)), _) if nu_lexes_as_pipe(first.kind, text) => None,
        (_, Some((second, _))) if second.kind == SyntaxKind::Colon => Some(RecordSign::ColonSecond),
        (Some((_, text)), _) => is_spread(text, Target::Record).then_some(RecordSign::SpreadFirst),
        (None, _) => Some(RecordSign::Empty),
    }
}

/// Whether nu's lexer makes a pipe of the token: `|`, `||`, and what it reports as a mistaken pipe.
fn nu_lexes_as_pipe(kind: SyntaxKind, text: &str) -> bool {
    matches!(kind, SyntaxKind::Pipe | SyntaxKind::PipePipe)
        || matches!(text, "o>|" | "out>|" | "&&" | "2>&1")
}

/// An item of a record as nu splits it.
enum RecordItem {
    Spread,
    Key,
    /// A key and the token nu takes as its `:`.
    KeyAndColon,
    /// A key, its `:`, and a value of this many touching tokens.
    Pair(usize),
}

/// The items of a record, given its inside in record mode.
/// `None` when the record stays a word: on a lexing error, or where nu's split cannot be made.
/// Nu lexes a key and its `:` as record mode does, but a value differently.
/// So a value is lexed again in its own mode.
/// It must cover touching tokens, with no comment among them.
fn split_record(inner: &Parser<'_>) -> Option<Vec<RecordItem>> {
    let tokens: Vec<_> = inner
        .remaining_tokens()
        .filter(|(token, _)| {
            !matches!(
                token.kind,
                SyntaxKind::Whitespace | SyntaxKind::Newline | SyntaxKind::Comma
            )
        })
        .collect();
    if tokens
        .iter()
        .any(|(token, _)| token.kind == SyntaxKind::Error)
    {
        return None;
    }
    let slot = |from: usize| {
        (from..tokens.len()).find(|&index| tokens[index].0.kind != SyntaxKind::Comment)
    };
    let mut items = Vec::new();
    let mut next = 0;
    while let Some(key) = slot(next) {
        if is_spread(tokens[key].1, Target::Record) {
            items.push(RecordItem::Spread);
            next = key + 1;
            continue;
        }
        let Some(colon) = slot(key + 1) else {
            items.push(RecordItem::Key);
            break;
        };
        let Some(start) = slot(colon + 1) else {
            items.push(RecordItem::KeyAndColon);
            break;
        };
        let value = inner.lex_first(tokens[start].0.range.start(), Mode::RecordValue)?;
        if value.kind == SyntaxKind::Error {
            return None;
        }
        let mut end = start;
        while tokens[end].0.range.end() < value.range.end() {
            end += 1;
            let (token, _) = tokens.get(end)?;
            if token.kind == SyntaxKind::Comment
                || token.range.start() != tokens[end - 1].0.range.end()
            {
                return None;
            }
        }
        if tokens[end].0.range.end() != value.range.end() {
            return None;
        }
        items.push(RecordItem::Pair(end - start + 1));
        next = end + 1;
    }
    Some(items)
}

/// The `items` that `split_record` gave for the tokens ahead.
/// Unlike `item`, reports no shell operator.
/// Keys are strings to nu, so a key and its `:` stay tokens.
fn record_items(p: &mut Parser<'_>, items: &[RecordItem]) {
    for item in items {
        bump_separators(p);
        if let RecordItem::Spread = item {
            spread(p, Target::Record);
            continue;
        }
        p.start(SyntaxKind::RecordField);
        p.bump();
        if matches!(item, RecordItem::KeyAndColon | RecordItem::Pair(_)) {
            bump_separators(p);
            p.bump();
        }
        match *item {
            RecordItem::Pair(1) => {
                bump_separators(p);
                value(p, Shape::Any);
            },
            RecordItem::Pair(count) => {
                bump_separators(p);
                p.bump_joined(count);
            },
            _ => {},
        }
        p.finish();
    }
    bump_separators(p);
}

pub(super) fn bump_separators(p: &mut Parser<'_>) {
    while matches!(p.peek(), Some(SyntaxKind::Newline | SyntaxKind::Comma)) {
        p.bump();
    }
}
