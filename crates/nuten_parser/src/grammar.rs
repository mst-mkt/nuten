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
            SyntaxKind::Pipe => p.bump(),
            kind if ends_statement(kind) => break,
            _ => pipeline_element(p),
        }
    }
    p.finish();
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
