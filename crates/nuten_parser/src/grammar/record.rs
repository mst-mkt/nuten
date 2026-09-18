use nuten_syntax::SyntaxKind;

use super::token_or_bracketed;
use crate::{
    lexer::Mode,
    parser::{Bracketed, Parser},
};

/// The `{ }` ahead that `is_record` as a `Record`, or as a word when its
/// tokens cannot make nu's split. `bracketed` is what it holds in record
/// mode.
pub(super) fn record<'a>(p: &mut Parser<'a>, bracketed: Bracketed<'a>) {
    match split_record(bracketed.inner()) {
        Some(items) => p.bump_bracketed(SyntaxKind::Record, bracketed, |p| {
            record_items(p, &items);
        }),
        None => p.bump(),
    }
}

/// Whether nu reads `{ }` as a record where it takes any value, given its
/// inside in brace mode: a `:` second or a spread first, unless a pipe comes
/// first.
pub(super) fn is_record(inner: &Parser<'_>) -> bool {
    let mut tokens = inner
        .remaining_tokens()
        .filter(|(token, _)| !token.kind.is_trivia());
    match (tokens.next(), tokens.next()) {
        (Some((first, text)), _) if is_pipe_to_nu(first.kind, text) => false,
        (_, Some((second, _))) if second.kind == SyntaxKind::Colon => true,
        (Some((_, text)), _) => is_spread(text),
        (None, _) => false,
    }
}

/// Whether nu's lexer makes a pipe of the token: `|`, `||`, and what it
/// reports as a mistaken pipe.
fn is_pipe_to_nu(kind: SyntaxKind, text: &str) -> bool {
    matches!(kind, SyntaxKind::Pipe | SyntaxKind::PipePipe)
        || matches!(text, "o>|" | "out>|" | "&&" | "2>&1")
}

/// Whether `text` spreads a record into one, as `...$x`, `...(f)` or
/// `...{a: 1}`.
fn is_spread(text: &str) -> bool {
    text.strip_prefix("...")
        .is_some_and(|rest| rest.starts_with(['$', '(', '{']))
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

/// The items of a record, given its inside in record mode, or `None` when
/// the record stays a word: on a lexing error, or where its tokens cannot
/// make nu's split. nu lexes a key and its `:` as record mode does, but a
/// value differently, so a value is lexed again in its own mode and must
/// cover touching tokens, with no comment among them.
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
        if is_spread(tokens[key].1) {
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
        let value = inner.lex_first(tokens[start].0.range.start(), Mode::record_value())?;
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

/// The `items` that `split_record` gave for the tokens ahead. Unlike `item`,
/// reports no error. Keys are strings to nu, so a key and its `:` stay
/// tokens.
fn record_items(p: &mut Parser<'_>, items: &[RecordItem]) {
    for item in items {
        bump_separators(p);
        if let RecordItem::Spread = item {
            p.bump();
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
                token_or_bracketed(p, false);
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

fn bump_separators(p: &mut Parser<'_>) {
    while matches!(p.peek(), Some(SyntaxKind::Newline | SyntaxKind::Comma)) {
        p.bump();
    }
}
