use nuten_syntax::SyntaxKind;
use rowan::{TextRange, TextSize};

use super::{
    PAREN, SQUARE, Shape, call_items,
    signature::{Enclosing, params},
    value,
};
use crate::{
    lexer::{Mode, Token},
    parser::{Affix, Brackets, Parser},
};

/// The `Def` ahead, when nu reads the call as one.
pub(super) fn def(p: &mut Parser<'_>) -> bool {
    let Some(shape) = def_shape(&call_items(p)) else {
        return false;
    };
    p.start(SyntaxKind::Def);
    for _ in 0..shape.signature {
        p.bump();
    }
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

/// The input and output types ahead: `count` items, with their `:` first unless it is attached.
/// They are an `Error` where nu fails to lex them.
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
    // Items with an error of their own keep it.
    let own_error = items
        .iter()
        .any(|(token, _)| token.kind == SyntaxKind::Error);
    let error = (!own_error)
        .then(|| p.lex_error(lexed, Mode::IoTypes))
        .flatten();
    if let Some(error) = error {
        p.bump_error_to(range.end(), error.into());
    } else {
        for _ in 0..count {
            p.bump();
        }
    }
}

/// Where `items` are, and where the part of them that nu lexes is.
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

/// Where the parts of a `def` or an `extern` are among `items`, when nu reads them as one.
/// The keyword, with `export` before it if there, then flags, the name, and the `[ ]` or `( )`.
/// Then the input and output types after a `:`, and last the body.
/// Only a `def` has a body, and nu takes it from the last item.
struct DefShape {
    /// Where the parameters are, the items before them being the keyword, any flags, and the name.
    signature: usize,
    brackets: &'static Brackets,
    /// Whether the `:` of the input and output types ends that item.
    attached_colon: bool,
    /// How many items of the input and output types follow the parameters, a lone `:` included.
    types: usize,
    /// Whether an `extern` ends with the one item in braces nu ignores.
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

/// The brackets `item` is enclosed in, when nu takes it for the parameters.
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
    fn signature_brackets_pair_opener_with_its_own_closer() {
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
