//! Grammar rules of Nushell.

use nuten_syntax::SyntaxKind;

use crate::parser::Parser;

pub(crate) fn root(p: &mut Parser) {
    p.start(SyntaxKind::Root);
    while !p.at_end() {
        p.bump();
    }
    p.finish();
}
