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
    while p.peek().is_some_and(|kind| !ends_statement(kind)) {
        p.bump();
    }
    p.finish();
}

fn ends_statement(kind: SyntaxKind) -> bool {
    matches!(kind, SyntaxKind::Newline | SyntaxKind::Semicolon)
}
