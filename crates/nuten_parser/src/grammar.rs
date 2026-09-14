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
        while p.peek() == Some(SyntaxKind::Newline) {
            p.bump();
        }
    }
}

fn next_line_continues(p: &Parser) -> bool {
    let mut kinds = p
        .remaining_kinds()
        .filter(|&kind| kind != SyntaxKind::Whitespace);
    while kinds.any(|kind| kind == SyntaxKind::Newline) {
        match kinds.next() {
            Some(SyntaxKind::Comment) => {},
            Some(kind) => return !ends_statement(kind),
            None => return false,
        }
    }
    false
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
