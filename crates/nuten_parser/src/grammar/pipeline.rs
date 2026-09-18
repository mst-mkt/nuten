use nuten_syntax::SyntaxKind;

use super::{
    Shape,
    assignment::{assigned_pipeline, assignment_ahead, var_type_error},
    command::{Streams, arguments, command},
    ends_statement, is_pipe,
};
use crate::{ParseErrorKind, parser::Parser};

/// `assigned` is whether an assignment operator takes the pipeline.
pub(super) fn pipeline(p: &mut Parser<'_>, assigned: bool) {
    p.start(SyntaxKind::Pipeline);
    let mut element = None;
    let mut first = true;
    while let Some(kind) = p.peek() {
        match kind {
            kind if is_pipe(kind) => pipe(p, kind, element.take(), assigned),
            SyntaxKind::Newline if next_line_starts_with_pipe(p) => bump_newlines(p),
            kind if ends_statement(kind) => break,
            _ => {
                element = pipeline_element(p, first);
                first = false;
            },
        }
    }
    p.finish();
}

/// `element` is what the element before the pipe redirects;
/// `None` when it has no command. `assigned` is as for `pipeline`.
fn pipe(p: &mut Parser<'_>, kind: SyntaxKind, element: Option<Streams>, assigned: bool) {
    match kind {
        SyntaxKind::Pipe if pipe_ends_file(p) => p.bump_error(ParseErrorKind::PipeAtEof),
        SyntaxKind::Pipe if assigned && pipe_ends_statement(p) => {
            p.bump_error(ParseErrorKind::PipeEndsAssignment);
        },
        SyntaxKind::Pipe => {
            p.bump();
            line_end_continuation(p);
        },
        // Nu goes on to the next line only after a plain `|`.
        _ => match element {
            None => p.bump_error(ParseErrorKind::RedirectionAtHead),
            Some(redirected) if redirected.overlaps(Streams::redirected_by(kind)) => {
                p.bump_error(ParseErrorKind::MultipleRedirections);
            },
            Some(_) => p.bump(),
        },
    }
}

/// Whether the `|` ahead is the last token of the file, past a trailing
/// comment and comment lines right below it: `Comment? (Newline Comment)*`.
/// A blank line or a final line break saves it; nu counts the `|` as the
/// last token only then.
fn pipe_ends_file(p: &Parser<'_>) -> bool {
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

/// Whether the statement ends at the line break right after the `|` ahead,
/// past a trailing comment.
fn pipe_ends_statement(p: &Parser<'_>) -> bool {
    p.remaining_kinds()
        .filter(|&kind| !matches!(kind, SyntaxKind::Whitespace | SyntaxKind::Comment))
        .nth(1)
        == Some(SyntaxKind::Newline)
        && !next_line_continues(p)
}

/// Whether a `|` starts the next line, past any comment lines.
/// It joins the line before it, along with those comment lines.
pub(super) fn next_line_starts_with_pipe(p: &Parser<'_>) -> bool {
    line_start(&mut next_lines(p)) == Some(SyntaxKind::Pipe)
}

/// After a `|` at the end of a line, the pipeline goes on at the next line,
/// past any comment lines. A blank line ends it, unless a line-start `|`
/// joins that blank line. `;` ends it too; nu rejects it after `|` anyway,
/// and this keeps the newline out of the pipeline, which must not end with
/// trivia.
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

pub(super) fn bump_newlines(p: &mut Parser<'_>) {
    while p.peek() == Some(SyntaxKind::Newline) {
        p.bump();
    }
}

/// Returns the streams the element redirects; `None` when it has no command.
/// `first` is whether no element comes before it in the pipeline.
fn pipeline_element(p: &mut Parser<'_>, first: bool) -> Option<Streams> {
    p.start(SyntaxKind::PipelineElement);
    let redirected = if assignment_ahead(p) {
        if first && let Some(keyword @ ("let" | "mut" | "const")) = p.peek_text() {
            p.start(SyntaxKind::Let);
            p.bump();
            if let Some((end, error)) = var_type_error(p, keyword) {
                p.bump();
                p.bump_error_to(end, error);
            }
            arguments(p, Some(Streams::default()), Shape::Argument, true);
        } else {
            p.start(SyntaxKind::Assignment);
            command(p, true);
        }
        p.bump();
        assigned_pipeline(p);
        p.finish();
        // No pipe follows what the assignment operator takes.
        None
    } else {
        command(p, false)
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
