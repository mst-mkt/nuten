use nuten_syntax::SyntaxKind;

use super::{file, shell_operator};
use crate::parser::Parser;

/// Whether `{ }` that is no record reads as a block or a closure, given its
/// inside in block mode: something but trivia, parameters that close at a
/// `|` with no `;` or shell operator before it, and statements that report
/// no error, since nothing inside may be reported.
pub(super) fn is_block(inner: &Parser<'_>) -> bool {
    if !inner.remaining_kinds().any(|kind| !kind.is_trivia()) {
        return false;
    }
    if params_start(inner) == Some(SyntaxKind::Pipe) {
        let closes = inner
            .remaining_tokens()
            .skip_while(|(token, _)| token.kind != SyntaxKind::Pipe)
            .skip(1)
            .find(|&(token, text)| {
                matches!(token.kind, SyntaxKind::Semicolon | SyntaxKind::Pipe)
                    || shell_operator(text).is_some()
            })
            .is_some_and(|(token, _)| token.kind == SyntaxKind::Pipe);
        if !closes {
            return false;
        }
    }
    !inner.reports_error(block_items)
}

/// The `|` or `||` that starts parameters where nu finds it: first past
/// whitespace, or a `|` past one line break, which nu's lexer makes that `|`.
fn params_start(p: &Parser<'_>) -> Option<SyntaxKind> {
    let mut kinds = p
        .remaining_kinds()
        .filter(|&kind| kind != SyntaxKind::Whitespace);
    match kinds.next() {
        Some(SyntaxKind::Newline) => kinds.next().filter(|&kind| kind == SyntaxKind::Pipe),
        first => first.filter(|&kind| matches!(kind, SyntaxKind::Pipe | SyntaxKind::PipePipe)),
    }
}

/// Parameters, if any, closing at the next `|`, then statements.
pub(super) fn block_items(p: &mut Parser<'_>) {
    match params_start(p) {
        Some(SyntaxKind::PipePipe) => {
            p.start(SyntaxKind::ClosureParams);
            p.bump();
            p.finish();
        },
        Some(SyntaxKind::Pipe) => {
            if p.peek() == Some(SyntaxKind::Newline) {
                p.bump();
            }
            p.start(SyntaxKind::ClosureParams);
            p.bump();
            while p.peek() != Some(SyntaxKind::Pipe) {
                p.bump();
            }
            p.bump();
            p.finish();
        },
        _ => {},
    }
    file(p);
}
