use std::mem;

use nuten_syntax::SyntaxKind;

use super::{
    Shape,
    alias::{alias, alias_ahead},
    assignment::{Assigned, assigned_statements, assignment_ahead},
    attribute::{attribute_ahead, attributed},
    builtin,
    command::{Streams, arguments, command},
    declaration, ends_statement, is_pipe,
    var_type::var_type,
};
use crate::{ParseErrorKind, parser::Parser};

pub(super) fn pipeline(p: &mut Parser<'_>, assigned: Assigned) {
    p.start(SyntaxKind::Pipeline);
    let mut element = Ended::default();
    let mut first = true;
    let mut piped = false;
    while let Some(kind) = p.peek() {
        match kind {
            kind if is_pipe(kind) => {
                pipe(p, kind, mem::take(&mut element).redirected, assigned);
                piped = true;
            },
            SyntaxKind::Newline if next_line_starts_with_pipe(p) => bump_newlines(p),
            SyntaxKind::Newline if element.ends_with_attribute && pipe_after_attribute(p) => {
                bump_newlines(p);
            },
            kind if ends_statement(kind) => break,
            _ => {
                element = pipeline_element(p, first, first && !piped, assigned);
                first = false;
            },
        }
    }
    p.finish();
}

#[derive(Default)]
pub(super) struct Ended {
    pub(super) redirected: Option<Streams>,
    pub(super) ends_with_attribute: bool,
}

/// After an attribute, nu's lite parser goes on to an `e>|` or an `o+e>|` starting the next line.
/// It goes on to a line-start `|` after a blank line too.
/// Its lexer drops only the line break of the blank line before that `|`, not the one ahead.
fn pipe_after_attribute(p: &Parser<'_>) -> bool {
    let mut lines = next_lines(p);
    match lines.next() {
        Some(SyntaxKind::Newline) => line_start(&mut lines) == Some(SyntaxKind::Pipe),
        Some(kind) => matches!(kind, SyntaxKind::ErrGtPipe | SyntaxKind::OutErrGtPipe),
        None => false,
    }
}

fn pipe(p: &mut Parser<'_>, kind: SyntaxKind, redirected: Option<Streams>, assigned: Assigned) {
    match kind {
        SyntaxKind::Pipe => plain_pipe(p, assigned),
        // Nu goes on to the next line only after a plain `|`.
        _ => match redirected {
            None => p.bump_error(ParseErrorKind::RedirectionAtHead),
            Some(streams) if streams.overlaps(Streams::redirected_by(kind)) => {
                p.bump_error(ParseErrorKind::MultipleRedirections);
            },
            Some(_) => p.bump(),
        },
    }
}

pub(super) fn plain_pipe(p: &mut Parser<'_>, assigned: Assigned) {
    if pipe_ends_file(p) {
        p.bump_error(ParseErrorKind::PipeAtEof);
    } else if pipe_ends_assigned(p, assigned) {
        p.bump_error(ParseErrorKind::PipeEndsAssignment);
    } else {
        p.bump();
        line_end_continuation(p);
    }
}

/// Nu counts the `|` as the last token only where no blank line or final line break follows it.
/// Where the parser skips a line break, nu drops every comment too.
pub(super) fn pipe_ends_file(p: &Parser<'_>) -> bool {
    if p.is_skipped(SyntaxKind::Newline) {
        return nothing_follows_pipe(p);
    }
    let mut kinds = p
        .remaining_kinds()
        .filter(|&kind| kind != SyntaxKind::Whitespace)
        .skip(1)
        .peekable();
    kinds.next_if_eq(&SyntaxKind::Comment);
    while kinds.next_if_eq(&SyntaxKind::Newline).is_some() {
        if kinds.next() != Some(SyntaxKind::Comment) {
            return false;
        }
    }
    kinds.next().is_none()
}

/// A `;` right after the `|` is left to `SemicolonAfterPipe`.
fn pipe_ends_assigned(p: &Parser<'_>, assigned: Assigned) -> bool {
    match assigned {
        Assigned::No => false,
        Assigned::AtLineBreak => {
            after_pipe(p).next() == Some(SyntaxKind::Newline) && !next_line_continues(p)
        },
        Assigned::AtSemicolon => nothing_follows_pipe(p),
    }
}

fn nothing_follows_pipe(p: &Parser<'_>) -> bool {
    after_pipe(p).all(|kind| kind == SyntaxKind::Newline)
}

fn after_pipe(p: &Parser<'_>) -> impl Iterator<Item = SyntaxKind> {
    p.remaining_kinds()
        .filter(|&kind| !p.is_skipped(kind))
        .skip(1)
}

pub(super) fn next_line_starts_with_pipe(p: &Parser<'_>) -> bool {
    line_start(&mut next_lines(p)) == Some(SyntaxKind::Pipe)
}

/// A line break the pipeline does not go past stays out of it, as a node must not end with trivia.
fn line_end_continuation(p: &mut Parser<'_>) {
    if p.peek() == Some(SyntaxKind::Newline) && next_line_continues(p) {
        bump_newlines(p);
    }
}

fn next_line_continues(p: &Parser<'_>) -> bool {
    let mut lines = next_lines(p);
    match line_start(&mut lines) {
        Some(SyntaxKind::Newline) => line_start(&mut lines) == Some(SyntaxKind::Pipe),
        Some(kind) => !ends_statement(kind),
        None => false,
    }
}

fn next_lines(p: &Parser<'_>) -> impl Iterator<Item = SyntaxKind> {
    p.remaining_kinds()
        .filter(|&kind| kind != SyntaxKind::Whitespace)
        .skip_while(|&kind| kind != SyntaxKind::Newline)
        .skip(1)
}

pub(super) fn line_start(lines: &mut impl Iterator<Item = SyntaxKind>) -> Option<SyntaxKind> {
    let mut first = lines.next();
    while first == Some(SyntaxKind::Comment) {
        lines.next();
        first = lines.next();
    }
    first
}

/// After an assignment operator, nu's lite parser takes the rest of the statement into one call.
/// So a `|` or a redirection is an argument.
/// So is `||`, which nu rejects only where it splits a pipeline.
pub(super) fn rest_of_statement<'a>(
    p: &mut Parser<'a>,
    assigned: Assigned,
    mut read_item: impl FnMut(&mut Parser<'a>),
) {
    while let Some(kind) = p.peek() {
        match kind {
            SyntaxKind::Pipe => plain_pipe(p, assigned),
            SyntaxKind::Newline if next_line_starts_with_pipe(p) => bump_newlines(p),
            kind if ends_statement(kind) => break,
            kind if kind.is_item() => read_item(p),
            _ => p.bump(),
        }
    }
}

pub(super) fn bump_newlines(p: &mut Parser<'_>) {
    while p.peek() == Some(SyntaxKind::Newline) {
        p.bump();
    }
}

/// Nu declares a variable or an alias only in the first element.
fn pipeline_element(
    p: &mut Parser<'_>,
    first: bool,
    heads_pipeline: bool,
    assigned: Assigned,
) -> Ended {
    p.start(SyntaxKind::PipelineElement);
    // Nu reads an attribute where a pipeline starts, or after a line break its lexer leaves.
    // It does so before it looks up an alias, even one of the same name.
    if attribute_ahead(p)
        && (heads_pipeline || p.last_unskipped_kind() == Some(SyntaxKind::Newline))
        && let Some(ended) = attributed(p, heads_pipeline, assigned)
    {
        p.finish();
        return ended;
    }
    let redirected = if first && alias_ahead(p) {
        alias(p, assigned);
        None
    } else if !assignment_ahead(p) {
        command(p, false)
    } else if !p.can_nest() {
        // What an assignment operator takes may hold another, and so on with no brackets.
        statement_as_written(p, assigned);
        None
    } else if first && let Some((keyword, words)) = declaration(p) {
        p.start(SyntaxKind::Let);
        p.bump_n(words);
        var_type(p, keyword);
        arguments(p, Some(Streams::default()), Shape::VariableName, true);
        assignment_operator(p);
        p.finish();
        None
    } else if first
        && p.peek_text()
            .is_some_and(|head| builtin::reads_before_assignment(head, p.nth_text(1)))
    {
        // Nu reads the syntax of the keyword, where the operator is only one more item.
        // Nu accepts that only where the operator is a name, or where the rest is left unread.
        // So the items stay as written.
        statement_as_written(p, assigned);
        None
    } else {
        p.start(SyntaxKind::Assignment);
        command(p, true);
        assignment_operator(p);
        p.finish();
        None
    };
    p.finish();
    Ended {
        redirected,
        ends_with_attribute: false,
    }
}

fn statement_as_written(p: &mut Parser<'_>, assigned: Assigned) {
    p.start(SyntaxKind::Call);
    rest_of_statement(p, assigned, Parser::bump);
    p.finish();
}

fn assignment_operator(p: &mut Parser<'_>) {
    p.bump();
    p.deeper(assigned_statements);
}

#[cfg(test)]
mod tests {
    use crate::{ParseErrorKind, parse};

    fn errors(src: &str) -> Vec<ParseErrorKind> {
        parse(src).errors().iter().map(|error| error.kind).collect()
    }

    #[test]
    fn pipe_at_eof_depends_on_line_breaks() {
        assert_eq!(errors("ls |"), [ParseErrorKind::PipeAtEof]);
        assert_eq!(errors(";|"), [ParseErrorKind::PipeAtEof]);
        assert_eq!(errors("ls | # c"), [ParseErrorKind::PipeAtEof]);
        assert_eq!(errors("ls |\n# c"), [ParseErrorKind::PipeAtEof]);
        assert_eq!(errors("ls | # c\n# d"), [ParseErrorKind::PipeAtEof]);
        assert_eq!(errors("ls |\n"), []);
        assert_eq!(errors("ls |\n# c\n"), []);
        assert_eq!(errors("ls |\n\n# c"), []);
    }

    #[test]
    fn redirecting_pipe_at_eof_has_no_error() {
        assert_eq!(errors("ls e>|"), []);
    }

    #[test]
    fn pipe_before_semicolon_reports_semicolon_after_pipe() {
        assert_eq!(errors("ls | ;"), [ParseErrorKind::SemicolonAfterPipe]);
    }

    #[test]
    fn pipe_at_eof_covers_last_pipe() {
        let parsed = parse("ls | | # c");
        assert_eq!(parsed.errors().len(), 1);
        assert_eq!(&"ls | | # c"[parsed.errors()[0].range], "|");
        assert_eq!(parsed.errors()[0].range.start(), 5.into());
    }
}
