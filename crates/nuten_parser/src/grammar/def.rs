use nuten_syntax::SyntaxKind;

use super::{
    PAREN, SQUARE, Shape, call_items,
    signature::{Enclosing, params},
    value,
};
use crate::{
    lexer::Mode,
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
    // The input and output types stay as they are; only the body is a block.
    let types = shape.rest - usize::from(shape.has_body);
    for at in 0..types {
        // The `:` nu takes for the one the types follow.
        // The formatter moves it to the parameters.
        if at == 0 && !shape.attached_colon && p.peek_text() == Some(":") {
            p.bump_as(SyntaxKind::Colon);
        } else {
            p.bump();
        }
    }
    if shape.has_body {
        value(p, Shape::DefBody);
    }
    p.finish();
    true
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
    /// How many items follow the parameters, the last being the body when the definition has one.
    rest: usize,
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
    let ignored = external && rest == 1 && items[items.len() - 1].starts_with('{');
    let types = rest - usize::from(has_body || ignored);
    if !attached_colon && types > 0 && items[signature + 1] != ":" {
        return None;
    }
    Some(DefShape {
        signature,
        brackets,
        attached_colon,
        rest,
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
