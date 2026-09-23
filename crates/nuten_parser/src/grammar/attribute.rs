use std::iter;

use nuten_syntax::SyntaxKind;

use super::{
    Shape,
    assignment::{Assigned, is_assignment_operator},
    at_call_item, builtin,
    command::{Streams, argument_shape, arguments, call, start_call},
    ends_statement, is_file_redirection, is_pipe, item,
    keyword::keyword,
    pipeline::{Ended, bump_newlines, line_start, next_line_starts_with_pipe, pipe_ends_file},
};
use crate::{ParseErrorKind, lexer::Token, parser::Parser};

/// Whether an item that starts with `@` stands ahead.
pub(super) fn attribute_ahead(p: &Parser<'_>) -> bool {
    p.peek().zip(p.peek_text()).is_some_and(heads_attribute)
}

/// Whether the token of `kind` and `text` may head an attribute.
fn heads_attribute((kind, text): (SyntaxKind, &str)) -> bool {
    kind.is_item() && text.starts_with('@')
}

/// The element that the attribute ahead heads.
/// `heads_pipeline` is whether no `|` comes before it.
/// `assigned` tells where the statement ends, as for the pipeline.
/// Nu reads the attributes as such only where they head a statement of one element.
/// Elsewhere it reads the whole element as one call, which goes on past what ends each attribute.
///
/// `None` when it has read attributes for no `def` or `extern`, and what ends the last of them.
/// The command after them is then left to the element.
pub(super) fn attributed(
    p: &mut Parser<'_>,
    heads_pipeline: bool,
    assigned: Assigned,
) -> Option<Ended> {
    if !heads_pipeline {
        return Some(attributed_call(p, assigned));
    }
    let past = past_attributes(lite_tokens(p), assigned);
    if past.piped {
        return Some(attributed_call(p, assigned));
    }
    if past.defines {
        return Some(attribute_block(p, assigned));
    }
    attributes(
        p,
        assigned,
        Some(ParseErrorKind::AttributeWithoutDefinition),
    );
    if after_attribute(p, assigned) == After::Nothing {
        return Some(Ended::default());
    }
    p.bump();
    None
}

/// The attributes ahead, with the `;` or line break between each two.
/// `error` is what the head of the last one reports, unless that head has an error of its own.
fn attributes(p: &mut Parser<'_>, assigned: Assigned, error: Option<ParseErrorKind>) {
    loop {
        let last = !attribute_follows(p, assigned);
        let reported = error.filter(|_| last && p.peek() != Some(SyntaxKind::Error));
        attribute(p, reported);
        if last {
            break;
        }
        p.bump();
    }
}

/// Whether another attribute of the element follows the one ahead.
fn attribute_follows(p: &Parser<'_>, assigned: Assigned) -> bool {
    let mut tokens = lite_tokens(p).skip_while(|&(kind, _)| !ends_statement(kind));
    tokens.next().is_some_and(|(end, _)| !assigned.ends_at(end))
        && tokens.next().is_some_and(heads_attribute)
}

/// The `AttributeBlock` ahead, where a `def` or an `extern` follows the attributes.
fn attribute_block(p: &mut Parser<'_>, assigned: Assigned) -> Ended {
    p.start(SyntaxKind::AttributeBlock);
    attributes(p, assigned, None);
    p.bump();
    let shape = argument_shape(p);
    if !keyword(p) {
        call(p, shape);
    }
    p.finish();
    Ended {
        redirected: arguments(p, Some(Streams::default()), shape, false),
        ends_with_attribute: false,
    }
}

/// The `Attribute` ahead, its head reporting `error` if given.
/// Nu looks up `@name` as the command `attr name`.
fn attribute(p: &mut Parser<'_>, error: Option<ParseErrorKind>) {
    let name = p.peek_text().map_or("", |head| &head[1..]);
    let shape = if builtin::is_attribute(name) {
        Shape::KnownArgument
    } else {
        Shape::UnknownArgument
    };
    p.start(SyntaxKind::Attribute);
    match error {
        Some(error) => p.bump_error(error),
        None => p.bump(),
    }
    attribute_arguments(p, shape);
    p.finish();
}

/// The items after the head of an attribute, as `shape`.
/// Nu's lite parser takes every token up to the end of the line as an argument, a `|` too.
/// Unlike after a `|` in a pipeline, a line break right after one ends the attribute.
/// Nu reports a `|` last in the file even so.
fn attribute_arguments(p: &mut Parser<'_>, shape: Shape) {
    while let Some(kind) = p.peek() {
        match kind {
            SyntaxKind::Newline if next_line_starts_with_pipe(p) => bump_newlines(p),
            kind if ends_statement(kind) => break,
            SyntaxKind::Pipe if pipe_ends_file(p) => p.bump_error(ParseErrorKind::PipeAtEof),
            kind if kind.is_item() => item(p, shape),
            _ => p.bump(),
        }
    }
}

/// The element ahead as one call: the attributes, what ends each, then the items of the command.
/// Nu runs the head as a command of that name.
fn attributed_call(p: &mut Parser<'_>, assigned: Assigned) -> Ended {
    let shape = argument_shape(p);
    start_call(p, shape);
    let after = loop {
        attribute_arguments(p, shape);
        match after_attribute(p, assigned) {
            After::Attribute => p.bump(),
            after => break after,
        }
    };
    if matches!(after, After::Command | After::Pipe) {
        p.bump();
    }
    while at_call_item(p) {
        item(p, shape);
    }
    p.finish();
    // A line break stays out of the call, which it would end.
    if after == After::Redirection {
        p.bump();
    }
    // Where what ends the attribute ends what an assignment operator takes, no `|` goes on past it.
    let ends_with_attribute =
        after == After::Nothing && !p.peek().is_some_and(|end| assigned.ends_at(end));
    Ended {
        redirected: arguments(p, Some(Streams::default()), shape, false),
        ends_with_attribute,
    }
}

/// What follows the `;` or line break that ends an attribute, as nu's lite parser reads it.
#[derive(PartialEq, Eq)]
enum After {
    /// Another attribute.
    Attribute,
    /// The items of the command.
    Command,
    /// A pipe right after a `;`, which the call then holds.
    Pipe,
    /// A file redirection, which nu applies to the attributes as a command.
    Redirection,
    /// Nothing more of the element.
    Nothing,
}

/// What follows the `;` or line break ahead, which ends an attribute.
/// A pipe after a line break is left to the pipeline, since the line break cannot end the call.
fn after_attribute(p: &Parser<'_>, assigned: Assigned) -> After {
    let mut tokens = lite_tokens(p);
    let end = tokens.next().map(|(kind, _)| kind);
    if end.is_none_or(|end| assigned.ends_at(end)) {
        return After::Nothing;
    }
    match tokens.next() {
        Some((kind, _)) if is_pipe(kind) && end == Some(SyntaxKind::Semicolon) => After::Pipe,
        Some((kind, _)) if is_file_redirection(kind) => After::Redirection,
        Some(token) if heads_attribute(token) => After::Attribute,
        Some((kind, _)) if kind.is_item() => After::Command,
        _ => After::Nothing,
    }
}

/// The kinds and texts of the tokens ahead as nu's lite parser gets them, trivia aside.
/// Nu's lexer drops a line break before a line-start `|`, and those of comment lines before it.
fn lite_tokens<'a>(p: &Parser<'a>) -> impl Iterator<Item = (SyntaxKind, &'a str)> + Clone {
    let skips_newline = p.is_skipped(SyntaxKind::Newline);
    let mut rest = p.remaining_tokens();
    iter::from_fn(move || {
        loop {
            let (token, text) = rest.next()?;
            let kept = match token.kind {
                SyntaxKind::Newline => !skips_newline && !lines_start_with_pipe(rest.clone()),
                kind => !kind.is_trivia(),
            };
            if kept {
                return Some((token.kind, text));
            }
        }
    })
}

/// Whether the first line of `lines`, past any comment lines, starts with `|`.
fn lines_start_with_pipe<'a>(lines: impl Iterator<Item = (Token, &'a str)>) -> bool {
    let mut kinds = lines
        .map(|(token, _)| token.kind)
        .filter(|&kind| kind != SyntaxKind::Whitespace);
    line_start(&mut kinds) == Some(SyntaxKind::Pipe)
}

/// What nu's lite parser takes past the attributes of an element.
#[derive(Default)]
struct PastAttributes {
    /// Whether the command is a `def` or an `extern`, `export` or not.
    defines: bool,
    /// Whether a pipe follows the element in the same pipeline.
    piped: bool,
}

/// What follows the attributes that `tokens` start with.
/// The attributes run to a `;` or a line break, and more of them may follow.
/// Past them, an assignment operator takes the rest of the statement, a `|` too.
fn past_attributes<'a>(
    tokens: impl Iterator<Item = (SyntaxKind, &'a str)> + Clone,
    assigned: Assigned,
) -> PastAttributes {
    let mut tokens = tokens.peekable();
    loop {
        if tokens
            .find(|&(kind, _)| ends_statement(kind))
            .is_none_or(|(end, _)| assigned.ends_at(end))
        {
            return PastAttributes::default();
        }
        if tokens.next_if(|&token| heads_attribute(token)).is_none() {
            break;
        }
    }
    let mut head = tokens
        .clone()
        .map_while(|(kind, text)| kind.is_item().then_some(text));
    let defines = matches!(
        (head.next(), head.next()),
        (Some("def" | "extern"), _) | (Some("export"), Some("def" | "extern"))
    );
    let kinds = tokens.map(|(kind, _)| kind);
    let piped = iter::once(None)
        .chain(kinds.clone().map(Some))
        .zip(kinds)
        .find(|&(prev, kind)| {
            is_pipe(kind) || ends_statement(kind) || is_assignment_operator(kind, prev)
        })
        .is_some_and(|(_, kind)| is_pipe(kind));
    PastAttributes {
        defines,
        piped,
    }
}
