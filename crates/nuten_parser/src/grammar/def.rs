use nuten_syntax::SyntaxKind;

use super::{SQUARE, Shape, signature::params, value};
use crate::{
    lexer::Mode,
    parser::{Affix, Parser},
};

/// Where the parts of a `def` or an `extern` are among `items`, when nu reads
/// them as one: the keyword, `export` before it if there, then flags, the
/// name, the parameters in `[ ]`, the input and output types after a `:`, and
/// the body, which only a `def` has and which nu takes from the last item.
pub(super) struct DefShape {
    /// Where the parameters are, the items before them being the keyword,
    /// any flags, and the name.
    signature: usize,
    /// Whether the `:` of the input and output types ends that item.
    attached_colon: bool,
    /// How many items follow the parameters, the last being the body when the
    /// definition has one.
    rest: usize,
    has_body: bool,
}

pub(super) fn def_shape(items: &[&str]) -> Option<DefShape> {
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
    // Nu reads a name holding `[` or `(` as the name and the parameters at
    // once, and rejects it.
    if items.get(at)?.contains(['[', '(']) {
        return None;
    }
    let signature = at + 1;
    let item = items.get(signature)?;
    let attached_colon = item.ends_with("]:");
    if !item.starts_with('[') || !(item.ends_with(']') || attached_colon) {
        return None;
    }
    let rest = items.len() - signature - 1;
    let has_body = !external && rest > 0;
    if has_body && !items[items.len() - 1].starts_with('{') {
        return None;
    }
    // Nu ignores the one item in braces an `extern` may end with, and never
    // parses it, so it stays a word.
    let ignored = external && rest == 1 && items[items.len() - 1].starts_with('{');
    let types = rest - usize::from(has_body || ignored);
    if !attached_colon && types > 0 && items[signature + 1] != ":" {
        return None;
    }
    Some(DefShape {
        signature,
        attached_colon,
        rest,
        has_body,
    })
}

/// The `Def` of `shape`, which `def_shape` gave for the items ahead.
pub(super) fn def(p: &mut Parser<'_>, shape: &DefShape) {
    p.start(SyntaxKind::Def);
    for _ in 0..shape.signature {
        p.bump();
    }
    let affix = if shape.attached_colon {
        Affix::Suffix(SyntaxKind::Colon)
    } else {
        Affix::None
    };
    match p.bracketed_affixed(&SQUARE, Mode::signature(), affix) {
        Some(bracketed) => p.bump_bracketed(SyntaxKind::Signature, bracketed, params),
        None => p.bump(),
    }
    // The input and output types stay as they are; only the body is a block.
    let types = shape.rest - usize::from(shape.has_body);
    for at in 0..types {
        // The `:` nu takes for the one the types follow, which the formatter
        // moves to the parameters.
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
}
