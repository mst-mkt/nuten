//! Grammar rules of Nushell.

use nuten_syntax::SyntaxKind;

use crate::parser::Parser;

pub(crate) fn file(p: &mut Parser) {
    while let Some(kind) = p.peek() {
        if ends_statement(kind) {
            p.bump();
        } else {
            pipeline(p);
        }
    }
}

fn pipeline(p: &mut Parser) {
    p.start(SyntaxKind::Pipeline);
    while let Some(kind) = p.peek() {
        match kind {
            SyntaxKind::Pipe => {
                p.bump();
                line_end_continuation(p);
            },
            kind if ends_statement(kind) => break,
            _ => pipeline_element(p),
        }
    }
    p.finish();
}

/// After a `|` at the end of a line, the pipeline goes on at the next line,
/// past any comment lines. A blank line ends it. So does `;`, which nu
/// rejects after `|` anyway: this keeps the newline out of the pipeline,
/// which must not end with trivia.
fn line_end_continuation(p: &mut Parser) {
    if p.peek() == Some(SyntaxKind::Newline) && next_line_continues(p) {
        bump_newlines(p);
    }
}

fn next_line_continues(p: &Parser) -> bool {
    line_start(&mut next_lines(p)).is_some_and(|kind| !ends_statement(kind))
}

/// The tokens after the current line, whitespace left out.
fn next_lines(p: &Parser) -> impl Iterator<Item = SyntaxKind> {
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

fn bump_newlines(p: &mut Parser) {
    while p.peek() == Some(SyntaxKind::Newline) {
        p.bump();
    }
}

fn pipeline_element(p: &mut Parser) {
    p.start(SyntaxKind::PipelineElement);
    while p.peek().is_some_and(|kind| !ends_element(kind)) {
        p.bump();
    }
    p.finish();
}

fn ends_statement(kind: SyntaxKind) -> bool {
    matches!(kind, SyntaxKind::Newline | SyntaxKind::Semicolon)
}

fn ends_element(kind: SyntaxKind) -> bool {
    kind == SyntaxKind::Pipe || ends_statement(kind)
}
