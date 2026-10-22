use nuten_syntax::SyntaxKind;
use rowan::{TextRange, TextSize};

use super::{
    PAREN, SQUARE, Shape, call_items,
    signature::{Enclosing, params, type_},
    value,
};
use crate::{
    lexer::{Mode, Token},
    parser::{Affix, Bracketed, Brackets, Parser},
};

pub(super) fn def(p: &mut Parser<'_>) -> bool {
    let Some(shape) = def_shape(&call_items(p)) else {
        return false;
    };
    p.start(SyntaxKind::Def);
    p.bump_n(shape.signature);
    let affix = if shape.attached_colon {
        Affix::Suffix(SyntaxKind::Colon)
    } else {
        Affix::None
    };
    match p.bracketed_affixed(shape.brackets, Mode::Signature, affix) {
        Some(bracketed) => p.bump_bracketed(SyntaxKind::Signature, bracketed, |p| {
            params(p, Enclosing::Brackets);
        }),
        None => p.bump(),
    }
    io_types(p, shape.types, shape.attached_colon);
    if shape.ignored_braces {
        p.bump();
    }
    if shape.has_body {
        value(p, Shape::DefBody);
    }
    p.finish();
    true
}

fn io_types(p: &mut Parser<'_>, count: usize, attached_colon: bool) {
    if count == 0 {
        return;
    }
    let count = if attached_colon {
        count
    } else {
        // A `Colon` of its own, which the formatter may join to the parameters.
        p.bump_as(SyntaxKind::Colon);
        count - 1
    };
    let items: Vec<_> = p
        .remaining_tokens()
        .filter(|(token, _)| !p.is_skipped(token.kind))
        .take(count)
        .collect();
    let Some((range, lexed)) = io_types_ranges(&items) else {
        return;
    };
    let own_error = items
        .iter()
        .any(|(token, _)| token.kind == SyntaxKind::Error);
    let error = (!own_error)
        .then(|| p.lex_error(lexed, Mode::IoTypes))
        .flatten();
    let bare = lexed == range;
    if let Some(error) = error {
        p.bump_error_to(range.end(), error.into());
    } else if let Some(bracketed) = (count == 1).then(|| bracketed_io_types(p)).flatten() {
        p.bump_bracketed(SyntaxKind::IoTypes, bracketed, io_type_pairs);
    } else if bare && bare_types_pair_up(p, range) {
        p.start(SyntaxKind::IoTypes);
        p.bump_relexed_to(range.end(), Mode::IoTypes, io_type_pairs);
        p.finish();
    } else {
        p.bump_n(count);
    }
}

/// Nu leaves out a `[` first and a `]` last, each on its own, before it lexes them.
fn io_types_ranges(items: &[(Token, &str)]) -> Option<(TextRange, TextRange)> {
    let (&(first, first_text), &(last, last_text)) = (items.first()?, items.last()?);
    let range = TextRange::new(first.range.start(), last.range.end());
    let lexed = TextRange::new(
        range.start() + TextSize::from(u32::from(first_text.starts_with('['))),
        range.end() - TextSize::from(u32::from(last_text.ends_with(']'))),
    );
    Some((range, lexed))
}

fn bracketed_io_types<'a>(p: &Parser<'a>) -> Option<Bracketed<'a>> {
    let bracketed = p.bracketed(&SQUARE, Mode::IoTypes)?;
    pairs_up(bracketed.inner()).then_some(bracketed)
}

/// Lexed as io types, a `#` after a `,` starts a comment that runs past the word.
/// A `,` alone would print as nothing, and leave the `:` with no types after it.
fn bare_types_pair_up(p: &Parser<'_>, range: TextRange) -> bool {
    let lexed = p.lexed_range(range, Mode::IoTypes);
    !lexed.has_glued_comment()
        && lexed.remaining_kinds().any(SyntaxKind::is_item)
        && pairs_up(&lexed)
}

/// Nu lexes a `,` as whitespace there.
/// One inside a pair keeps the types as they are, so that a pair holds its three items alone.
fn pairs_up(p: &Parser<'_>) -> bool {
    let part = p
        .remaining_tokens()
        .filter(|(token, _)| !p.is_skipped(token.kind))
        .try_fold(0, |part, (token, text)| match (token.kind, part) {
            (SyntaxKind::Comma, 0) => Some(0),
            (kind, _) if !kind.is_item() || kind == SyntaxKind::Error => None,
            (_, 1) if text != "->" => None,
            _ => Some((part + 1) % 3),
        });
    part == Some(0)
}

fn io_type_pairs(p: &mut Parser<'_>) {
    while let Some(kind) = p.peek() {
        if kind == SyntaxKind::Comma {
            p.bump();
        } else {
            p.start(SyntaxKind::IoType);
            type_(p);
            p.bump();
            type_(p);
            p.finish();
        }
    }
}

/// Only a `def` has a body, and nu takes it from the last item.
struct DefShape {
    signature: usize,
    brackets: &'static Brackets,
    attached_colon: bool,
    types: usize,
    ignored_braces: bool,
    has_body: bool,
}

fn def_shape(items: &[&str]) -> Option<DefShape> {
    let mut at = usize::from(*items.first()? == "export");
    let external = match *items.get(at)? {
        "def" => false,
        "extern" => true,
        _ => return None,
    };
    at += 1;
    while items.get(at)?.starts_with('-') {
        at += 1;
    }
    // Nu reads a name holding `[` or `(` as the name and the parameters at once, and rejects it.
    if items.get(at)?.contains(['[', '(']) {
        return None;
    }
    let signature = at + 1;
    let item = *items.get(signature)?;
    let attached_colon = item.ends_with(':');
    let brackets = signature_brackets(item.strip_suffix(':').unwrap_or(item))?;
    let rest = items.len() - signature - 1;
    let has_body = !external && rest > 0;
    if has_body && !items[items.len() - 1].starts_with('{') {
        return None;
    }
    // Nu ignores the one item in braces an `extern` may end with, and never parses it.
    // So it stays a word.
    let ignored_braces = external && rest == 1 && items[items.len() - 1].starts_with('{');
    let types = rest - usize::from(has_body || ignored_braces);
    if !attached_colon && types > 0 && items[signature + 1] != ":" {
        return None;
    }
    Some(DefShape {
        signature,
        brackets,
        attached_colon,
        types,
        ignored_braces,
        has_body,
    })
}

/// Nu takes `[ ]` and `( )` alike, each opener only with its own closer.
fn signature_brackets(item: &str) -> Option<&'static Brackets> {
    [&SQUARE, &PAREN]
        .into_iter()
        .find(|brackets| brackets.wrap(item))
}

#[cfg(test)]
mod tests {
    use super::signature_brackets;

    #[test]
    fn signature_brackets_match_and_wrap_item() {
        for (item, expected) in [
            ("[]", Some(('[', ']'))),
            ("()", Some(('(', ')'))),
            ("(x: int)", Some(('(', ')'))),
            ("[x)", None),
            ("(x]", None),
            ("[", None),
            ("]", None),
            ("(x)y", None),
            ("x", None),
            ("", None),
        ] {
            let brackets = signature_brackets(item)
                .map(|brackets| (char::from(brackets.open), char::from(brackets.close)));
            assert_eq!(brackets, expected, "{item:?}");
        }
    }
}
