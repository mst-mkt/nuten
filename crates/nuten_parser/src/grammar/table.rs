use nuten_syntax::SyntaxKind;

use super::{
    SQUARE,
    list::{is_list, list_items},
};
use crate::{lexer::Mode, parser::Parser};

/// Whether the `[ ]` is a table, given its inside in list mode: the column names in `[ ]` first, `;` right after them, and rows in `[ ]`, any number of `,` and comments among them.
/// Nu reads a table past a `,` or a comment before the `;` too, but such a `[ ]` stays a word, as pulling the `;` up to the names would put it after the comment.
/// A `;` later is an error to nu, and it reads `[[a b];]` as a list, so those stay words as well.
pub(super) fn is_table(inner: &Parser<'_>) -> bool {
    let mut tokens = inner
        .remaining_tokens()
        .filter(|(token, _)| !matches!(token.kind, SyntaxKind::Whitespace | SyntaxKind::Newline));
    let in_brackets =
        |kind: SyntaxKind, text: &str| kind == SyntaxKind::Word && text.starts_with('[');
    let header = tokens
        .next()
        .is_some_and(|(token, text)| in_brackets(token.kind, text));
    let semicolon = tokens
        .next()
        .is_some_and(|(token, _)| token.kind == SyntaxKind::Semicolon);
    let mut rows = 0;
    header
        && semicolon
        && tokens.all(|(token, text)| match token.kind {
            SyntaxKind::Comma | SyntaxKind::Comment => true,
            kind if in_brackets(kind, text) => {
                rows += 1;
                true
            },
            _ => false,
        })
        && rows > 0
}

/// The column names, the `;`, and the rows, each a list, or a word when it holds a `;`: nu reads no table there, and drops what follows the `;`.
pub(super) fn table_items(p: &mut Parser<'_>) {
    while let Some(kind) = p.peek() {
        if let Some(bracketed) = p.bracketed(&SQUARE, Mode::List)
            && is_list(bracketed.inner())
        {
            p.bump_bracketed(SyntaxKind::List, bracketed, list_items);
        } else {
            debug_assert!(
                !matches!(kind, SyntaxKind::Error | SyntaxKind::Comment),
                "{kind:?} is not among the items of a table"
            );
            p.bump();
        }
    }
}
