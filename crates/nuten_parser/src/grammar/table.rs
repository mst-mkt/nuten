use nuten_syntax::SyntaxKind;

use super::{
    SQUARE, Shape,
    list::is_list,
    spread::{Target, is_spread},
    value,
};
use crate::{ParseErrorKind, lexer::Mode, parser::Parser};

/// Whether the `[ ]` is a table, given its inside in list mode.
/// The column names in `[ ]` first, `;` right after them, then rows in `[ ]`.
/// Any number of `,` and comments may come among the rows.
/// Nu reads a table past a `,` or a comment before the `;` too, but such a `[ ]` stays a word.
/// Pulling the `;` up to the names would put it after the comment.
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

/// The column names, the `;`, and the rows, each a list.
/// One holding a `;` is a word instead: nu reads no table there, and drops what follows.
pub(super) fn table_items(p: &mut Parser<'_>) {
    while let Some(kind) = p.peek() {
        if let Some(bracketed) = p.bracketed(&SQUARE, Mode::List)
            && is_list(bracketed.inner())
        {
            p.bump_bracketed(SyntaxKind::List, bracketed, row_items);
        } else {
            debug_assert!(
                !matches!(kind, SyntaxKind::Error | SyntaxKind::Comment),
                "{kind:?} is not among the items of a table"
            );
            p.bump();
        }
    }
}

/// The items of the column names or of a row.
/// Nu reports a spread there, as it takes none in a table.
fn row_items(p: &mut Parser<'_>) {
    while let Some(text) = p.peek_text() {
        if is_spread(text, Target::List) {
            p.bump_error(ParseErrorKind::SpreadInTable);
        } else {
            value(p, Shape::Any);
        }
    }
}
