use nuten_syntax::SyntaxKind;

use super::{
    Shape,
    alias::{alias, alias_ahead},
    assignment::{Assigned, assigned_statements, assignment_ahead},
    command::{Streams, arguments, command},
    declaration, ends_statement, is_pipe,
    var_type::var_type,
};
use crate::{ParseErrorKind, parser::Parser};

pub(super) fn pipeline(p: &mut Parser<'_>, assigned: Assigned) {
    p.start(SyntaxKind::Pipeline);
    let mut element = None;
    let mut first = true;
    let mut piped = false;
    while let Some(kind) = p.peek() {
        match kind {
            kind if is_pipe(kind) => {
                pipe(p, kind, element.take(), assigned);
                piped = true;
            },
            SyntaxKind::Newline if next_line_starts_with_pipe(p) => bump_newlines(p),
            kind if ends_statement(kind) => break,
            _ => {
                element = pipeline_element(p, first, first && !piped, assigned);
                first = false;
            },
        }
    }
    p.finish();
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

/// The `|` ahead, and the line breaks nu goes on past after it.
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

/// Whether the `|` ahead is the last token of the file.
/// A trailing comment and comment lines right below it may follow: `Comment? (Newline Comment)*`.
/// A blank line or a final line break saves it; nu counts the `|` as the last token only then.
/// Where the parser skips a line break, nu drops every comment too.
fn pipe_ends_file(p: &Parser<'_>) -> bool {
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

/// Whether the `|` ahead is the last token of what an assignment operator takes.
/// A `;` right after it is left to `SemicolonAfterPipe`.
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

/// The kinds after the `|` ahead, the skipped ones left out.
fn after_pipe(p: &Parser<'_>) -> impl Iterator<Item = SyntaxKind> {
    p.remaining_kinds()
        .filter(|&kind| !p.is_skipped(kind))
        .skip(1)
}

/// Whether a `|` starts the next line, past any comment lines.
/// It joins the line before it, along with those comment lines.
pub(super) fn next_line_starts_with_pipe(p: &Parser<'_>) -> bool {
    line_start(&mut next_lines(p)) == Some(SyntaxKind::Pipe)
}

/// After a `|` at the end of a line, the pipeline goes on at the next line, past any comment lines.
/// A blank line ends it, unless a line-start `|` joins that blank line.
/// `;` ends it too, and nu rejects it after `|` anyway.
/// Ending there keeps the newline out of the pipeline, which must not end with trivia.
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

/// The tokens after the current line, whitespace left out.
fn next_lines(p: &Parser<'_>) -> impl Iterator<Item = SyntaxKind> {
    p.remaining_kinds()
        .filter(|&kind| kind != SyntaxKind::Whitespace)
        .skip_while(|&kind| kind != SyntaxKind::Newline)
        .skip(1)
}

/// The first token of a line, past any comment lines.
/// `Newline` for a blank line, `None` at the end of the file.
fn line_start(lines: &mut impl Iterator<Item = SyntaxKind>) -> Option<SyntaxKind> {
    let mut first = lines.next();
    while first == Some(SyntaxKind::Comment) {
        lines.next();
        first = lines.next();
    }
    first
}

/// The tokens ahead up to the statement's end, each item read by `read_item`.
/// `assigned` tells where the statement ends, as for a pipeline.
/// After an assignment operator, nu's lite parser takes them all into one call.
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

/// Returns the streams the element redirects.
/// `None` when it has no command, or when it runs to the statement's end.
/// `first` is whether it is the first element, which is where nu declares a variable or an alias.
/// `heads_pipeline` is whether no `|` came before it either, which is where nu reads an attribute.
/// `assigned` tells where the statement ends, as for the pipeline.
fn pipeline_element(
    p: &mut Parser<'_>,
    first: bool,
    heads_pipeline: bool,
    assigned: Assigned,
) -> Option<Streams> {
    p.start(SyntaxKind::PipelineElement);
    let redirected = if first && alias_ahead(p) {
        alias(p, assigned);
        None
    } else if assignment_ahead(p) {
        if first && let Some((keyword, words)) = declaration(p) {
            p.start(SyntaxKind::Let);
            for _ in 0..words {
                p.bump();
            }
            var_type(p, keyword);
            arguments(p, Some(Streams::default()), Shape::VariableName, true);
        } else {
            p.start(SyntaxKind::Assignment);
            command(p, heads_pipeline, true);
        }
        p.bump();
        assigned_statements(p);
        p.finish();
        None
    } else {
        command(p, heads_pipeline, false)
    };
    p.finish();
    redirected
}

#[cfg(test)]
mod tests {
    use crate::{ParseErrorKind, parse};

    fn errors(src: &str) -> Vec<ParseErrorKind> {
        parse(src).errors().iter().map(|error| error.kind).collect()
    }

    #[test]
    fn pipe_at_eof_depends_on_a_final_line_break() {
        assert_eq!(errors("ls |"), [ParseErrorKind::PipeAtEof]);
        assert_eq!(errors(";|"), [ParseErrorKind::PipeAtEof]);
        assert_eq!(errors("ls | # c"), [ParseErrorKind::PipeAtEof]);
        assert_eq!(errors("ls |\n# c"), [ParseErrorKind::PipeAtEof]);
        assert_eq!(errors("ls | # c\n# d"), [ParseErrorKind::PipeAtEof]);
        assert_eq!(errors("ls |\n"), []);
        assert_eq!(errors("ls |\n# c\n"), []);
        assert_eq!(errors("ls |\n\n# c"), []);
        assert_eq!(errors("ls e>|"), []);
        assert_eq!(errors("ls | ;"), [ParseErrorKind::SemicolonAfterPipe]);
    }

    #[test]
    fn pipe_at_eof_is_reported_at_the_last_pipe() {
        let parsed = parse("ls | | # c");
        assert_eq!(parsed.errors().len(), 1);
        assert_eq!(&"ls | | # c"[parsed.errors()[0].range], "|");
        assert_eq!(parsed.errors()[0].range.start(), 5.into());
    }
}
