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

pub(super) fn attribute_ahead(p: &Parser<'_>) -> bool {
    p.peek().zip(p.peek_text()).is_some_and(heads_attribute)
}

fn heads_attribute((kind, text): (SyntaxKind, &str)) -> bool {
    kind.is_item() && text.starts_with('@')
}

/// Nu reads the attributes as such only where they head a statement of one element.
/// Elsewhere it reads the whole element as one call, which goes on past what ends each attribute.
/// On `None`, the command after the attributes is left to the caller.
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

fn attribute_follows(p: &Parser<'_>, assigned: Assigned) -> bool {
    let mut tokens = lite_tokens(p).skip_while(|&(kind, _)| !ends_statement(kind));
    tokens.next().is_some_and(|(end, _)| !assigned.ends_at(end))
        && tokens.next().is_some_and(heads_attribute)
}

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

#[derive(PartialEq, Eq)]
enum After {
    Attribute,
    Command,
    Pipe,
    /// A file redirection, which nu applies to the attributes as a command.
    Redirection,
    Nothing,
}

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

fn lines_start_with_pipe<'a>(lines: impl Iterator<Item = (Token, &'a str)>) -> bool {
    let mut kinds = lines
        .map(|(token, _)| token.kind)
        .filter(|&kind| kind != SyntaxKind::Whitespace);
    line_start(&mut kinds) == Some(SyntaxKind::Pipe)
}

#[derive(Default)]
struct PastAttributes {
    defines: bool,
    piped: bool,
}

/// Past the attributes, an assignment operator takes the rest of the statement, a `|` too.
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
