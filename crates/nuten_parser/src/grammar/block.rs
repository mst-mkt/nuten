use nuten_syntax::SyntaxKind;

use super::{
    file, shell_operator,
    signature::{Enclosing, params},
};
use crate::{
    lexer::Mode,
    parser::{Bracketed, Parser},
};

/// The `{ }` ahead that is no record, as a `Block`.
/// As a word when nu reads no block or closure in it.
/// `bracketed` is what it holds in block mode.
pub(super) fn block<'a>(p: &mut Parser<'a>, bracketed: Bracketed<'a>) {
    if may_be_block(bracketed.inner()) {
        p.bump_bracketed_or_word(SyntaxKind::Block, bracketed, block_items);
    } else {
        p.bump();
    }
}

/// Whether the inside of `{ }` in block mode has the shape of a block or a closure.
/// It must hold something but trivia.
/// Its parameters must close at a `|` with no `;` or shell operator before it.
/// Its statements must report no error either, since nothing inside may be reported.
/// Only reading the statements tells that.
fn may_be_block(inner: &Parser<'_>) -> bool {
    if !inner.remaining_kinds().any(|kind| !kind.is_trivia()) {
        return false;
    }
    params_start(inner) != Some(SyntaxKind::Pipe) || params_close(inner)
}

/// Whether the `|` that starts parameters has another `|` after it.
/// No `;` or shell operator may come between them.
fn params_close(inner: &Parser<'_>) -> bool {
    inner
        .remaining_tokens()
        .skip_while(|(token, _)| token.kind != SyntaxKind::Pipe)
        .skip(1)
        .find(|&(token, text)| {
            matches!(token.kind, SyntaxKind::Semicolon | SyntaxKind::Pipe)
                || shell_operator(text).is_some()
        })
        .is_some_and(|(token, _)| token.kind == SyntaxKind::Pipe)
}

/// The `|` or `||` that starts parameters where nu finds it.
/// First past whitespace, or a `|` past one line break, which nu's lexer makes that `|`.
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
/// Nu lexes the parameters again and reads them as those of a `def`.
fn block_items(p: &mut Parser<'_>) {
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
            let close = p
                .remaining_tokens()
                .find(|(token, _)| token.kind == SyntaxKind::Pipe)
                .map(|(token, _)| token.range.start())
                .expect("parameters close at a `|`");
            p.bump_relexed_to(close, Mode::Signature, |p| params(p, Enclosing::Pipes));
            p.bump();
            p.finish();
        },
        _ => {},
    }
    file(p);
}
