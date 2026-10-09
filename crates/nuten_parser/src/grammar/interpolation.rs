use std::{iter, ops::Range};

use nuten_syntax::SyntaxKind;
use rowan::{TextRange, TextSize};

use super::{PAREN, file};
use crate::{
    lexer::Mode,
    parser::{Parser, Piece},
};

/// Nu reads a `( )` in an interpolation as it does one that stands where it takes a value.
pub(super) fn interpolation(p: &mut Parser<'_>) {
    let text = p.peek_text().expect("an interpolated string ahead");
    let start = p
        .peek_range()
        .expect("an interpolated string ahead")
        .start();
    let pieces = pieces(text, start);
    let split = p
        .split_token(SyntaxKind::InterpString, &pieces)
        .expect("pieces that cover the string ahead");
    p.bump_relexed(SyntaxKind::Interpolation, split, |p| {
        while let Some(range) = p.peek_range() {
            // Nu lexes the `( )` again as a cell path.
            if p.peek() == Some(SyntaxKind::Word)
                && let Some(error) = p.lex_error(range, Mode::CellPath)
            {
                p.bump_error_to(range.end(), error.into());
                continue;
            }
            match p.bracketed(&PAREN, Mode::Paren) {
                Some(bracketed) => p.bump_bracketed(SyntaxKind::Subexpression, bracketed, file),
                None => p.bump(),
            }
        }
    });
}

fn pieces(text: &str, start: TextSize) -> Vec<Piece> {
    let range = |bytes: Range<usize>| {
        let offset = |at: usize| start + TextSize::try_from(at).expect("an offset in a token");
        TextRange::new(offset(bytes.start), offset(bytes.end))
    };
    let parens = parens(text);
    let text_starts = iter::once(0).chain(parens.iter().map(|paren| paren.end));
    let text_ends = parens.iter().map(|paren| paren.start).chain([text.len()]);
    let texts = text_starts.zip(text_ends).map(|(from, to)| {
        (from < to).then(|| Piece::Token(SyntaxKind::InterpText, range(from..to)))
    });
    let words = parens
        .iter()
        .map(|paren| Some(Piece::Token(SyntaxKind::Word, range(paren.clone()))))
        .chain([None]);
    texts
        .zip(words)
        .flat_map(|(text, word)| [text, word])
        .flatten()
        .collect()
}

/// Ported from nu's `parse_string_interpolation`.
pub(super) fn parens(text: &str) -> Vec<Range<usize>> {
    let bytes = text.as_bytes();
    let escapes = bytes[1] == b'"';
    let end = bytes.len() - 1;
    let mut parens = Vec::new();
    let mut closers: Vec<u8> = Vec::new();
    let mut open = 0;
    let mut backslashes = 0;
    let mut at = 2;
    while at < end {
        let byte = bytes[at];
        match closers.last() {
            None if byte == b'(' && (!escapes || backslashes % 2 == 0) => {
                open = at;
                closers.push(b')');
            },
            None => backslashes = if byte == b'\\' { backslashes + 1 } else { 0 },
            Some(&b'"') if byte == b'\\' && at + 1 < end => at += 1,
            Some(&closer) if closer != b')' => {
                if byte == closer {
                    closers.pop();
                }
            },
            Some(_) => match byte {
                b'\'' | b'"' | b'`' => closers.push(byte),
                b'(' => closers.push(b')'),
                b')' => {
                    closers.pop();
                    if closers.is_empty() {
                        parens.push(open..at + 1);
                        backslashes = 0;
                    }
                },
                _ => {},
            },
        }
        at += 1;
    }
    parens
}

#[cfg(test)]
mod tests {
    use super::parens;

    fn parens_of(text: &str) -> Vec<&str> {
        parens(text).into_iter().map(|range| &text[range]).collect()
    }

    #[test]
    fn parens_are_subexpressions_of_string() {
        assert_eq!(parens_of(r#"$"a(1 + 2)b(c)""#), ["(1 + 2)", "(c)"]);
        assert_eq!(parens_of(r#"$"(a)(b)""#), ["(a)", "(b)"]);
        assert_eq!(parens_of("$'(a)'"), ["(a)"]);
        assert_eq!(parens_of(r#"$"a""#), Vec::<&str>::new());
        assert_eq!(parens_of(r#"$"()""#), ["()"]);
    }

    #[test]
    fn odd_backslashes_escape_paren_only_in_double_quotes() {
        assert_eq!(parens_of(r#"$"\(a)""#), Vec::<&str>::new());
        assert_eq!(parens_of(r#"$"\\(a)""#), ["(a)"]);
        assert_eq!(parens_of(r#"$"\\\(a)""#), Vec::<&str>::new());
        assert_eq!(parens_of(r"$'\(a)'"), ["(a)"]);
    }

    #[test]
    fn parens_nest_and_quotes_hide_parens() {
        assert_eq!(parens_of(r#"$"((a) b)""#), ["((a) b)"]);
        assert_eq!(parens_of(r#"$"(')' + ")")""#), [r#"(')' + ")")"#]);
        assert_eq!(parens_of(r#"$"("\")" + `)`)""#), [r#"("\")" + `)`)"#]);
        assert_eq!(parens_of(r#"$"('\' + 'a')""#), [r"('\' + 'a')"]);
    }

    #[test]
    fn backslashes_before_paren_count_from_last_subexpression() {
        assert_eq!(parens_of(r#"$"(a)\(b)""#), ["(a)"]);
        assert_eq!(parens_of(r#"$"\(a)(b)""#), ["(b)"]);
    }
}
