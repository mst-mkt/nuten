use std::iter;

use nuten_syntax::SyntaxKind;

use super::{CURLY, SQUARE, Shape, record::bump_separators, value, variable::is_variable_name};
use crate::{
    ParseErrorKind,
    lexer::{Mode, Token},
    parser::Parser,
};

/// Nu tells the pattern by its first byte.
pub(super) fn pattern(p: &mut Parser<'_>) {
    match p.peek_text().map(str::as_bytes) {
        Some([b'$', ..]) => variable(p, "$"),
        Some([b'{', ..]) => record_pattern(p),
        Some([b'[', ..]) => list_pattern(p),
        _ => value(p, Shape::Any),
    }
}

/// Nu takes a `$` alone for a name in a pattern, unlike where it reads a variable as a value.
fn variable(p: &mut Parser<'_>, prefix: &str) {
    let name = p
        .peek_text()
        .and_then(|text| text.strip_prefix(prefix))
        .expect("a variable ahead");
    if name.is_empty() || is_variable_name(name) {
        p.bump();
    } else {
        p.bump_error(ParseErrorKind::InvalidVariableName);
    }
}

fn pattern_tokens<'a>(inner: &Parser<'a>) -> Vec<(Token, &'a str)> {
    inner
        .remaining_tokens()
        .filter(|(token, _)| !token.kind.is_trivia() && token.kind != SyntaxKind::Comma)
        .collect()
}

/// Unlike for `is_item`, an `Error` is none, since nuten reads no pattern of its text.
fn is_pattern_item(kind: SyntaxKind) -> bool {
    kind.is_item() && kind != SyntaxKind::Error
}

enum Split {
    Items,
    /// Nu reads a token nuten lexes as no item, such as `|`, as a string in a record pattern.
    Tokens,
    Rejected,
}

fn record_pattern(p: &mut Parser<'_>) {
    let Some(bracketed) = p.bracketed(&CURLY, Mode::RecordPattern) else {
        return p.bump();
    };
    match split(bracketed.inner()) {
        Split::Items => {
            p.bump_bracketed(SyntaxKind::RecordPattern, bracketed, record_pattern_fields);
        },
        Split::Tokens => p.bump(),
        Split::Rejected => p.bump_error(ParseErrorKind::RecordPatternKey),
    }
}

fn split(inner: &Parser<'_>) -> Split {
    let tokens = pattern_tokens(inner);
    let fields: Vec<_> =
        iter::successors(Field::first(&tokens), |field| Field::first(field.rest)).collect();
    let rest = fields.last().map_or(tokens.as_slice(), |field| field.rest);
    if !rest.is_empty() {
        Split::Rejected
    } else if fields
        .iter()
        .flat_map(|field| field.parts)
        .all(|(token, _)| is_pattern_item(token.kind))
    {
        Split::Items
    } else {
        Split::Tokens
    }
}

struct Field<'t, 'a> {
    parts: [&'t (Token, &'a str); 2],
    rest: &'t [(Token, &'a str)],
}

impl<'t, 'a> Field<'t, 'a> {
    fn first(tokens: &'t [(Token, &'a str)]) -> Option<Field<'t, 'a>> {
        let (parts, rest) = match tokens {
            [variable @ (_, text), rest @ ..] if text.starts_with('$') => {
                ([variable, variable], rest)
            },
            [key, (colon, _), pattern, rest @ ..] if colon.kind == SyntaxKind::Colon => {
                ([key, pattern], rest)
            },
            _ => return None,
        };
        Some(Field {
            parts,
            rest,
        })
    }
}

fn record_pattern_fields(p: &mut Parser<'_>) {
    while let Some((kind, text)) = p.peek().zip(p.peek_text()) {
        if matches!(kind, SyntaxKind::Comma | SyntaxKind::Newline) {
            p.bump();
        } else if text.starts_with('$') {
            variable(p, "$");
        } else {
            record_field(p);
        }
    }
}

fn record_field(p: &mut Parser<'_>) {
    p.start(SyntaxKind::RecordField);
    p.bump();
    bump_separators(p);
    p.bump();
    bump_separators(p);
    pattern(p);
    p.finish();
}

/// Nu splits the patterns at a `|`, rejects a `;`, and reads none past a redirection.
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

fn reads_whole(inner: &Parser<'_>) -> bool {
    let tokens = pattern_tokens(inner);
    let rest = tokens.iter().position(|&(_, text)| is_rest(text));
    tokens.iter().all(|(token, _)| is_pattern_item(token.kind))
        && rest.is_none_or(|rest| rest + 1 == tokens.len())
}

fn is_rest(text: &str) -> bool {
    text == ".." || text.starts_with("..$")
}

fn list_pattern_items(p: &mut Parser<'_>) {
    while let Some((kind, text)) = p.peek().zip(p.peek_text()) {
        if matches!(kind, SyntaxKind::Comma | SyntaxKind::Newline) || text == ".." {
            p.bump();
        } else if is_rest(text) {
            variable(p, "..$");
        } else {
            pattern(p);
        }
    }
}
