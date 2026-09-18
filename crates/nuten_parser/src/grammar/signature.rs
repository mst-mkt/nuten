use nuten_syntax::SyntaxKind;

use super::is_item;
use crate::{
    ParseErrorKind,
    lexer::Mode,
    parser::{Affix, Brackets, Parser},
};

/// What a signature takes next, as nu's `parse_signature_helper` calls it.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Params {
    /// A parameter, a flag, a `:`, an `=`, or a `,`.
    Param,
    /// A parameter or a flag, after a `,`.
    AfterComma,
    /// The type a `:` promises.
    Type,
    /// A parameter, a flag, an `=`, or a `,`, after a type.
    AfterType,
    /// The value an `=` promises.
    Default,
}

/// The parameters of a signature as nu splits them: a `Param` at each item
/// where nu takes one, holding the `(-f)`, the `:` and the type, and the `=`
/// and the default value that nu gives it, along with the comments that
/// describe it. A `:`, an `=`, or a `,` where nu takes none is an `Error`, as
/// is one with nothing after it.
pub(super) fn params(p: &mut Parser<'_>) {
    let mut mode = Params::Param;
    let mut started = false;
    while let Some(kind) = p.peek() {
        let promised = matches!(kind, SyntaxKind::Colon | SyntaxKind::Eq) && !token_ahead(p);
        mode = match (kind, mode) {
            (SyntaxKind::Colon, Params::Param) if !promised => Params::Type,
            (SyntaxKind::Eq, Params::Param | Params::AfterType) if !promised => Params::Default,
            (SyntaxKind::Comma, Params::Param | Params::AfterType) => Params::AfterComma,
            (SyntaxKind::Colon | SyntaxKind::Eq | SyntaxKind::Comma, _) => {
                p.bump_error(ParseErrorKind::SignatureSeparator);
                continue;
            },
            // Nu takes a `(-f)` for the parameter before it, and has none
            // after a `,`. An `Error` token already carries its error.
            (_, Params::AfterComma)
                if kind != SyntaxKind::Error && p.peek_text().is_some_and(is_short_flag) =>
            {
                p.bump_error(ParseErrorKind::SignatureSeparator);
                continue;
            },
            // A line break belongs to the parameter only while it goes on.
            (SyntaxKind::Newline, mode) => {
                if started && !goes_on(p, mode) {
                    p.finish();
                    started = false;
                }
                mode
            },
            (_, Params::Type) => {
                type_(p);
                mode = Params::AfterType;
                continue;
            },
            (_, Params::Default) => Params::Param,
            // An item that is no `(-f)` starts a parameter of its own; nu
            // ignores `|`, `;`, and redirections, which stay where they are.
            (_, _) if is_item(kind) && !p.peek_text().is_some_and(is_short_flag) => {
                if started {
                    p.finish();
                }
                // Trivia between two parameters, such as the comment that
                // describes the one before, belongs to neither.
                p.bump_skipped();
                p.start(SyntaxKind::Param);
                started = true;
                Params::Param
            },
            (_, mode) => mode,
        };
        p.bump();
    }
    if started {
        p.finish();
    }
}

/// Whether the parameter being read goes on past the line breaks ahead: nu
/// gives it the type or the default value it promised, the `(-f)`, the type,
/// or the default value that comes next, or the `,` that ends it.
fn goes_on(p: &Parser<'_>, mode: Params) -> bool {
    let next = p
        .remaining_tokens()
        .find(|(token, _)| !token.kind.is_trivia() && token.kind != SyntaxKind::Newline);
    match mode {
        Params::Type | Params::Default => next.is_some(),
        _ => next.is_some_and(|(token, text)| {
            matches!(
                token.kind,
                SyntaxKind::Colon | SyntaxKind::Eq | SyntaxKind::Comma
            ) || is_short_flag(text)
        }),
    }
}

const ANGLE: Brackets = Brackets {
    open: b'<',
    close: b'>',
    open_kind: SyntaxKind::LAngle,
    close_kind: SyntaxKind::RAngle,
    always_nested: false,
};

/// The type ahead: a `Type` where nu reads parameters in `< >`, or else the
/// item as it is. The name must be one of the four that take parameters, and
/// the `< >` must hold nothing but items, `:`, `,`, and whitespace, which
/// leaves out a comment, since printing the type on one line would put the
/// `>` inside it.
fn type_(p: &mut Parser<'_>) {
    let Some((name, named)) = generic_type(p.peek_text()) else {
        p.bump();
        return;
    };
    let Some(bracketed) = p.bracketed_affixed(&ANGLE, Mode::type_params(), Affix::Name(name))
    else {
        p.bump();
        return;
    };
    let takes = bracketed.inner().remaining_kinds().all(|kind| {
        (is_item(kind) && kind != SyntaxKind::Error)
            || matches!(
                kind,
                SyntaxKind::Colon
                    | SyntaxKind::Comma
                    | SyntaxKind::Whitespace
                    | SyntaxKind::Newline
            )
    });
    if !takes {
        p.bump();
        return;
    }
    let items = if named { type_fields } else { type_params };
    p.bump_bracketed(SyntaxKind::Type, bracketed, items);
}

/// The name of the type ahead and whether its parameters are fields, when nu
/// reads it as a type with parameters: one of `list`, `oneof`, `record`, and
/// `table`, then `< >` around the rest. A `@` anywhere moves where nu ends
/// the name, which takes a completer after it, so the `< >` here need not be
/// the one nu reads.
fn generic_type(text: Option<&str>) -> Option<(usize, bool)> {
    let text = text?;
    if text.contains('@') || !text.ends_with('>') {
        return None;
    }
    let name = text.find('<')?;
    match &text[..name] {
        "record" | "table" => Some((name, true)),
        "list" | "oneof" => Some((name, false)),
        _ => None,
    }
}

/// The types a `list` or a `oneof` takes, which nu reads one by one, `,`
/// aside.
fn type_params(p: &mut Parser<'_>) {
    while let Some(kind) = p.peek() {
        if matches!(kind, SyntaxKind::Comma | SyntaxKind::Newline) {
            p.bump();
        } else {
            type_(p);
        }
    }
}

/// The fields a `record` or a `table` takes: a key, then the `:` and the type
/// that follow it. Anything else starts a field of its own, as in nu.
fn type_fields(p: &mut Parser<'_>) {
    while let Some(kind) = p.peek() {
        if matches!(kind, SyntaxKind::Comma | SyntaxKind::Newline) {
            p.bump();
            continue;
        }
        p.start(SyntaxKind::TypeField);
        p.bump();
        if colon_ahead(p) {
            while p.peek() == Some(SyntaxKind::Newline) {
                p.bump();
            }
            p.bump();
            // Nu takes the token after the `:`, whatever it is, for the type
            // of the key, and a line break before it is whitespace to it.
            if p.remaining_kinds().any(|kind| !kind.is_trivia()) {
                while p.peek() == Some(SyntaxKind::Newline) {
                    p.bump();
                }
                type_(p);
            }
        }
        p.finish();
    }
}

/// Whether the key just read has a `:` after it, past line breaks, which
/// gives it the type that follows.
fn colon_ahead(p: &Parser<'_>) -> bool {
    p.remaining_kinds().find(|kind| !kind.is_trivia()) == Some(SyntaxKind::Colon)
}

/// Whether any token comes before the parameters end, a comment included.
/// Nu reports a `:` or an `=` only as the very last token, however little
/// what follows it has to do with a type or a default value.
fn token_ahead(p: &Parser<'_>) -> bool {
    p.remaining_kinds()
        .filter(|kind| !matches!(kind, SyntaxKind::Whitespace | SyntaxKind::Newline))
        .nth(1)
        .is_some()
}

/// Whether `text` is how nu spells a short flag for the parameter before it.
fn is_short_flag(text: &str) -> bool {
    text.starts_with("(-")
}
