//! Grammar rules of Nushell.

use nuten_syntax::SyntaxKind;

use crate::parser::Parser;

pub(crate) fn file(p: &mut Parser<'_>) {
    while let Some(kind) = p.peek() {
        if ends_statement(kind) {
            p.bump();
        } else {
            pipeline(p);
        }
    }
}

fn pipeline(p: &mut Parser<'_>) {
    p.start(SyntaxKind::Pipeline);
    while let Some(kind) = p.peek() {
        match kind {
            SyntaxKind::Pipe => {
                p.bump();
                line_end_continuation(p);
            },
            // Nu goes on to the next line only after a plain `|`.
            kind if is_pipe(kind) => p.bump(),
            SyntaxKind::Newline if next_line_starts_with_pipe(p) => bump_newlines(p),
            kind if ends_statement(kind) => break,
            _ => pipeline_element(p),
        }
    }
    p.finish();
}

/// Whether a `|` starts the next line, past any comment lines.
/// It joins the line before it, along with those comment lines.
fn next_line_starts_with_pipe(p: &Parser<'_>) -> bool {
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

fn bump_newlines(p: &mut Parser<'_>) {
    while p.peek() == Some(SyntaxKind::Newline) {
        p.bump();
    }
}

fn pipeline_element(p: &mut Parser<'_>) {
    p.start(SyntaxKind::PipelineElement);
    if !p.peek().is_some_and(is_file_redirection) {
        call(p);
    }
    while let Some(kind) = p.peek() {
        match kind {
            kind if ends_element(kind) => break,
            kind if is_file_redirection(kind) => redirection(p),
            _ => p.bump(),
        }
    }
    p.finish();
}

fn call(p: &mut Parser<'_>) {
    let kind = if p.peek_text().is_some_and(|head| head.starts_with('^')) {
        SyntaxKind::ExternalCall
    } else {
        SyntaxKind::Call
    };
    p.start(kind);
    while p.peek().is_some_and(|kind| !ends_call(kind)) {
        p.bump();
    }
    p.finish();
}

/// Nu wants an item right after the operator; anything else is left out.
fn redirection(p: &mut Parser<'_>) {
    p.start(SyntaxKind::Redirection);
    p.bump();
    if p.peek().is_some_and(is_item) {
        p.bump();
    }
    p.finish();
}

fn ends_statement(kind: SyntaxKind) -> bool {
    matches!(kind, SyntaxKind::Newline | SyntaxKind::Semicolon)
}

fn ends_element(kind: SyntaxKind) -> bool {
    is_pipe(kind) || ends_statement(kind)
}

fn ends_call(kind: SyntaxKind) -> bool {
    is_file_redirection(kind) || ends_element(kind)
}

fn is_pipe(kind: SyntaxKind) -> bool {
    matches!(
        kind,
        SyntaxKind::Pipe | SyntaxKind::ErrGtPipe | SyntaxKind::OutErrGtPipe
    )
}

fn is_file_redirection(kind: SyntaxKind) -> bool {
    matches!(
        kind,
        SyntaxKind::OutGt
            | SyntaxKind::OutGtGt
            | SyntaxKind::ErrGt
            | SyntaxKind::ErrGtGt
            | SyntaxKind::OutErrGt
            | SyntaxKind::OutErrGtGt
    )
}

fn is_item(kind: SyntaxKind) -> bool {
    matches!(
        kind,
        SyntaxKind::Word
            | SyntaxKind::String
            | SyntaxKind::RawString
            | SyntaxKind::InterpString
            | SyntaxKind::Error
    )
}
