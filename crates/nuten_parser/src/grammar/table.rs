use nuten_syntax::SyntaxKind;

use super::{
    SQUARE, Shape,
    list::{is_list, piped_items},
    spread::{Target, is_spread},
    value,
};
use crate::{ParseErrorKind, lexer::Mode, parser::Parser};

/// Nu reads a table past a `,` or a comment before the `;` too, but such a `[ ]` stays a word.
/// Pulling the `;` up to the names would put it after the comment.
/// Nu rejects a second `;`, and one with no row after it as in `[[a b];]`.
/// So those stay words as well.
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

/// A list holding a `;`, the column names or a row, stays a word, since nu rejects the `;`.
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

/// Nu reports a spread in a table, as it takes none there.
fn row_items(p: &mut Parser<'_>) {
    piped_items(p, |p| match p.peek_text() {
        Some(text) if is_spread(text, Target::List) => p.bump_error(ParseErrorKind::SpreadInTable),
        _ => value(p, Shape::Any),
    });
}

#[cfg(test)]
mod tests {
    use crate::{ParseErrorKind, parse};

    fn errors(src: &str) -> Vec<ParseErrorKind> {
        parse(src).errors().iter().map(|error| error.kind).collect()
    }

    #[test]
    fn pipes_in_header_or_row_report_as_in_list() {
        assert_eq!(errors("[[a |]; [1]]"), [ParseErrorKind::PipeEndsList]);
        assert_eq!(
            errors("[[a]; [e>| 1]]"),
            [ParseErrorKind::RedirectionAtHead]
        );
        assert_eq!(errors("[[a b]; [1 | 2]]"), []);
        assert_eq!(errors("[[@a | b]; [1 2 3]]"), []);
    }
}
