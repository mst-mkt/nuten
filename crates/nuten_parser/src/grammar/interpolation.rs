use std::{iter, ops::Range};

use nuten_syntax::SyntaxKind;
use rowan::{TextRange, TextSize};

use super::{PAREN, file};
use crate::{
    lexer::Mode,
    parser::{Parser, Piece},
};

/// The `InterpString` ahead as an `Interpolation`: its text, and each `( )` in it.
/// Nu reads a `( )` there as it does one that stands where it takes a value.
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
        while p.peek().is_some() {
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

/// The range of each `( )` in the interpolation `text`, brackets included.
/// Ported from nu's `parse_string_interpolation`.
/// In a `$"` string, a `(` after an odd number of `\` is text.
/// Inside a `( )`, quotes make nested strings, where only a `"` one takes a `\` escape.
pub(super) fn parens(text: &str) -> Vec<Range<usize>> {
    let bytes = text.as_bytes();
    let escapes = bytes[1] == b'"';
    // Past the `$` and the opening quote, and before the closing one.
    let end = bytes.len() - 1;
    let mut parens = Vec::new();
    // The closers expected inside the `( )` being read, innermost last.
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
