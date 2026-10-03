use nuten_syntax::SyntaxKind;

use super::{
    file, shell_operator,
    signature::{Enclosing, params},
};
use crate::{
    lexer::Mode,
    parser::{Bracketed, Parser},
};

/// `bracketed` must hold the inside of the `{ }` in block mode.
pub(super) fn block<'a>(p: &mut Parser<'a>, bracketed: Bracketed<'a>) {
    if may_be_block(bracketed.inner()) {
        p.bump_bracketed_or_word(SyntaxKind::Block, bracketed, block_items);
    } else {
        p.bump();
    }
}

/// The statements must report no error too, since nothing inside may be reported.
/// Only reading them tells that, so this checks the parameters alone.
fn may_be_block(inner: &Parser<'_>) -> bool {
    params_start(inner) != Some(SyntaxKind::Pipe) || params_close(inner)
}

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

/// A `|` past one line break starts parameters too, since nu's lexer reads it so.
pub(super) fn params_start(p: &Parser<'_>) -> Option<SyntaxKind> {
    let mut kinds = p
        .remaining_kinds()
        .filter(|&kind| kind != SyntaxKind::Whitespace);
    match kinds.next() {
        Some(SyntaxKind::Newline) => kinds.next().filter(|&kind| kind == SyntaxKind::Pipe),
        first => first.filter(|&kind| matches!(kind, SyntaxKind::Pipe | SyntaxKind::PipePipe)),
    }
}

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
