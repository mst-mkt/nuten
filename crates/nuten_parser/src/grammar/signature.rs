use nuten_syntax::SyntaxKind;

use super::pipeline::bump_newlines;
use crate::{
    ParseErrorKind,
    lexer::Mode,
    parser::{Affix, Bracketed, Brackets, Parser},
};

/// Ported from nu's `parse_signature_helper`.
#[derive(Clone, Copy)]
enum Expect {
    Param,
    AfterComma,
    Type,
    AfterType,
    Default,
}

enum Action {
    Bump,
    Reject,
    BumpAsShortFlag,
    ReadType,
    EndParam,
    StartParam,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Enclosing {
    Brackets,
    Pipes,
}

pub(super) fn params(p: &mut Parser<'_>, enclosing: Enclosing) {
    let mut expect = Expect::Param;
    let mut in_param = false;
    while let Some((action, next)) = step(p, expect, in_param, enclosing) {
        match action {
            Action::Bump => p.bump(),
            Action::Reject => p.bump_error(ParseErrorKind::SignatureSeparator),
            Action::BumpAsShortFlag => p.bump_as(SyntaxKind::ShortFlag),
            Action::ReadType => type_(p),
            Action::EndParam => {
                p.finish();
                p.bump();
                in_param = false;
            },
            Action::StartParam => {
                if in_param {
                    p.finish();
                }
                p.start(SyntaxKind::Param);
                p.bump();
                in_param = true;
            },
        }
        expect = next;
    }
    if in_param {
        p.finish();
    }
}

fn step(
    p: &Parser<'_>,
    expect: Expect,
    in_param: bool,
    enclosing: Enclosing,
) -> Option<(Action, Expect)> {
    let (kind, text) = p.peek().zip(p.peek_text())?;
    // The closing `|` of a closure is nu's last token, so a `:` or an `=` is never last there.
    let not_last = || enclosing == Enclosing::Pipes || token_ahead(p);
    Some(match (kind, expect) {
        (SyntaxKind::Colon, Expect::Param) if not_last() => (Action::Bump, Expect::Type),
        (SyntaxKind::Eq, Expect::Param | Expect::AfterType) if not_last() => {
            (Action::Bump, Expect::Default)
        },
        (SyntaxKind::Comma, Expect::Param | Expect::AfterType) => {
            (Action::Bump, Expect::AfterComma)
        },
        (SyntaxKind::Colon | SyntaxKind::Eq | SyntaxKind::Comma, _) => (Action::Reject, expect),
        // Nu takes a `(-f)` for the parameter before it, and has none after a `,`.
        // An `Error` token already carries its error.
        (_, Expect::AfterComma) if kind != SyntaxKind::Error && starts_short_flag(text) => {
            (Action::Reject, expect)
        },
        // A line break belongs to the parameter only while it goes on.
        (SyntaxKind::Newline, _) if in_param && !goes_on(p, expect) => (Action::EndParam, expect),
        (SyntaxKind::Newline, _) => (Action::Bump, expect),
        (_, Expect::Type) => (Action::ReadType, Expect::AfterType),
        (_, Expect::Default) => (Action::Bump, Expect::Param),
        (SyntaxKind::Word, _) if in_param && is_short_flag(text) => {
            (Action::BumpAsShortFlag, expect)
        },
        // Nu ignores `|`, `;`, and redirections, which stay where they are.
        _ if kind.is_item() && !starts_short_flag(text) => (Action::StartParam, Expect::Param),
        _ => (Action::Bump, expect),
    })
}

fn goes_on(p: &Parser<'_>, expect: Expect) -> bool {
    let next = p
        .remaining_tokens()
        .find(|(token, _)| !token.kind.is_trivia() && token.kind != SyntaxKind::Newline);
    match expect {
        Expect::Type | Expect::Default => next.is_some(),
        _ => next.is_some_and(|(token, text)| {
            matches!(
                token.kind,
                SyntaxKind::Colon | SyntaxKind::Eq | SyntaxKind::Comma
            ) || starts_short_flag(text)
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

pub(super) fn type_(p: &mut Parser<'_>) {
    let Some((bracketed, named)) = type_ahead(p) else {
        p.bump();
        return;
    };
    let items = if named { type_fields } else { type_params };
    p.bump_bracketed(SyntaxKind::Type, bracketed, items);
}

pub(super) fn is_type(p: &Parser<'_>) -> bool {
    type_ahead(p).is_some()
}

/// No comment may be inside, since printing the type on one line would put the `>` inside it.
fn type_ahead<'a>(p: &Parser<'a>) -> Option<(Bracketed<'a>, bool)> {
    let (name, named) = generic_type(p.peek_text())?;
    let bracketed = p.bracketed_affixed(&ANGLE, Mode::TypeParams, Affix::Name(name))?;
    let takes = bracketed.inner().remaining_kinds().all(|kind| {
        (kind.is_item() && kind != SyntaxKind::Error)
            || matches!(
                kind,
                SyntaxKind::Colon
                    | SyntaxKind::Comma
                    | SyntaxKind::Whitespace
                    | SyntaxKind::Newline
            )
    });
    takes.then_some((bracketed, named))
}

/// A `@` anywhere moves where nu ends the name, which takes a completer after it.
/// So the `< >` here need not be the one nu reads.
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

fn type_params(p: &mut Parser<'_>) {
    while let Some(kind) = p.peek() {
        if matches!(kind, SyntaxKind::Comma | SyntaxKind::Newline) {
            p.bump();
        } else {
            type_(p);
        }
    }
}

fn type_fields(p: &mut Parser<'_>) {
    while let Some(kind) = p.peek() {
        if matches!(kind, SyntaxKind::Comma | SyntaxKind::Newline) {
            p.bump();
            continue;
        }
        p.start(SyntaxKind::TypeField);
        p.bump();
        if colon_ahead(p) {
            bump_newlines(p);
            p.bump();
            // Nu takes the token after the `:` for the type of the key, whatever it is.
            // A line break before it is whitespace to nu.
            if p.remaining_kinds().any(|kind| !kind.is_trivia()) {
                bump_newlines(p);
                type_(p);
            }
        }
        p.finish();
    }
}

fn colon_ahead(p: &Parser<'_>) -> bool {
    p.remaining_kinds().find(|kind| !kind.is_trivia()) == Some(SyntaxKind::Colon)
}

/// Nu reports a `:` or an `=` only as the very last token.
/// What follows it need have nothing to do with a type or a default value.
fn token_ahead(p: &Parser<'_>) -> bool {
    p.remaining_kinds()
        .filter(|kind| !matches!(kind, SyntaxKind::Whitespace | SyntaxKind::Newline))
        .nth(1)
        .is_some()
}

/// Nu takes a `(-` for a short flag whatever it makes of the flag then.
fn starts_short_flag(text: &str) -> bool {
    text.starts_with("(-")
}

/// Nu rejects a short flag of other than one character.
/// Joined to its parameter, such a flag would be rejected differently, so it stays a word.
fn is_short_flag(text: &str) -> bool {
    text.strip_prefix("(-")
        .and_then(|rest| rest.strip_suffix(')'))
        .is_some_and(|name| name.chars().count() == 1)
}
