use nuten_syntax::SyntaxKind;
use rowan::{TextRange, TextSize};

use super::{SQUARE, Shape, is_file_redirection};
use crate::{
    ParseErrorKind,
    lexer::{self, Mode, Token},
    parser::{Affix, Bracketed, Parser},
};

/// Ported from nu's `parse_binary`, which takes only these, in lower case.
const PREFIXES: [(&str, u32, usize); 3] = [("0x", 16, 2), ("0o", 8, 3), ("0b", 2, 8)];

pub(super) enum BinaryAhead<'a> {
    Decoded(Bracketed<'a>),
    Undecodable,
}

/// Nu reads a binary only where it takes any value.
pub(super) fn binary_ahead<'a>(p: &Parser<'a>, shape: Shape) -> Option<BinaryAhead<'a>> {
    if !shape.takes_any_value() {
        return None;
    }
    let text = p.peek_text()?;
    let &(prefix, base, digits) = PREFIXES.iter().find(|(prefix, ..)| {
        text.strip_prefix(prefix)
            .is_some_and(|rest| rest.starts_with('['))
    })?;
    let bracketed = p.bracketed_affixed(&SQUARE, Mode::Binary, Affix::Name(prefix.len()))?;
    let tokens: Vec<_> = bracketed.inner().remaining_tokens().collect();
    if tokens
        .iter()
        .any(|&(token, text)| token.kind == SyntaxKind::Error || ends_item(token.kind, text))
    {
        return None;
    }
    if decode(&items(&tokens), base, digits).is_none() {
        return Some(BinaryAhead::Undecodable);
    }
    // Nu skips a `;` among the items, but the formatter would print it as an item.
    let holds_semicolon = tokens
        .iter()
        .any(|(token, _)| token.kind == SyntaxKind::Semicolon);
    (!holds_semicolon).then_some(BinaryAhead::Decoded(bracketed))
}

pub(super) fn binary<'a>(p: &mut Parser<'a>, ahead: BinaryAhead<'a>) {
    match ahead {
        BinaryAhead::Decoded(bracketed) => p.bump_bracketed(SyntaxKind::Binary, bracketed, |p| {
            while p.peek().is_some() {
                p.bump();
            }
        }),
        BinaryAhead::Undecodable => p.bump_error(ParseErrorKind::BinaryDigits),
    }
}

/// Nu's `is_math_expression_like` takes a binary for the start of an expression, decodable or not.
pub(super) fn is_binary_like(text: &str) -> bool {
    let Some(inside) = PREFIXES
        .iter()
        .find_map(|(prefix, ..)| text.strip_prefix(prefix)?.strip_prefix('['))
        .and_then(|rest| rest.strip_suffix(']'))
    else {
        return false;
    };
    let start = TextSize::of(text) - TextSize::of(inside) - TextSize::of("]");
    let range = TextRange::at(start, TextSize::of(inside));
    lexer::lex(text, range, Mode::Binary)
        .tokens
        .iter()
        .all(|&token| !ends_item(token.kind, &text[token.range]))
}

/// Nu reads no binary where it lexes a pipe, a redirection, or an assignment among the items.
fn ends_item(kind: SyntaxKind, text: &str) -> bool {
    matches!(
        kind,
        SyntaxKind::Pipe
            | SyntaxKind::PipePipe
            | SyntaxKind::ErrGtPipe
            | SyntaxKind::OutErrGtPipe
            | SyntaxKind::Assign
    ) || is_file_redirection(kind)
        || matches!(text, "o>|" | "out>|" | "&&" | "2>" | "2>&1")
}

fn items(tokens: &[(Token, &str)]) -> String {
    tokens
        .iter()
        .filter(|(token, _)| token.kind.is_item())
        .map(|&(_, text)| text)
        .collect()
}

/// Ported from nu's `parse_binary_with_base` and `decode_with_base`.
/// Nu pads the digits with `0`s at the front to a whole number of bytes, counting bytes of UTF-8.
/// It then cuts them into bytes by chars.
fn decode(joined: &str, base: u32, digits: usize) -> Option<Vec<u8>> {
    let padding = (digits - joined.len() % digits) % digits;
    let padded: Vec<char> = "0".repeat(padding).chars().chain(joined.chars()).collect();
    padded
        .chunks(digits)
        .map(|chunk| u8::from_str_radix(&chunk.iter().collect::<String>(), base).ok())
        .collect()
}

#[cfg(test)]
mod tests {
    use nuten_syntax::SyntaxKind;

    use super::{decode, is_binary_like};
    use crate::{ParseErrorKind, parse};

    #[test]
    fn digits_decode_as_nu_decodes_them() {
        for (joined, base, digits, expected) in [
            ("ff00", 16, 2, Some(vec![0xff, 0x00])),
            ("FF", 16, 2, Some(vec![0xff])),
            ("f", 16, 2, Some(vec![0x0f])),
            ("fff", 16, 2, Some(vec![0x0f, 0xff])),
            ("", 16, 2, Some(vec![])),
            ("+f", 16, 2, Some(vec![0x0f])),
            ("f+f", 16, 2, Some(vec![0x0f, 0x0f])),
            ("+ff", 16, 2, None),
            ("zz", 16, 2, None),
            ("f_f", 16, 2, None),
            ("日", 16, 2, None),
            ("377", 8, 3, Some(vec![0xff])),
            ("400", 8, 3, None),
            ("1", 8, 3, Some(vec![1])),
            ("0101", 2, 8, Some(vec![5])),
            ("2", 2, 8, None),
        ] {
            assert_eq!(decode(joined, base, digits), expected, "{joined:?}");
        }
    }

    #[test]
    fn binary_like_is_prefixed_brackets_without_pipe_redirection_or_assignment() {
        for text in [
            "0x[ff]",
            "0x[]",
            "0o[777]",
            "0b[2]",
            "0x[zz]",
            "0x[ff;00]",
            "0x[ff # c\n00]",
            "0x[ff][00]",
            "0x[\"ff\"]",
        ] {
            assert!(is_binary_like(text), "{text:?}");
        }
        for text in [
            "0X[ff]",
            "0x[ff",
            "0x[ff]a",
            "0x ff",
            "0x[ff|00]",
            "0x[ff || 00]",
            "0x[ff = 00]",
            "0x[ff o> 00]",
            "0x[ff e>| 00]",
            "0x[ff && 00]",
            "0x[ff 2> 00]",
            "0x[ff 2>&1 00]",
            "0x[ff o>| 00]",
            "0z[ff]",
        ] {
            assert!(!is_binary_like(text), "{text:?}");
        }
    }

    #[test]
    fn undecodable_binary_error_covers_whole_word() {
        for src in ["0x[+ff]", "0x[\"ff\"]", "0x[zz;]"] {
            let parsed = parse(src);
            let errors: Vec<_> = parsed
                .errors()
                .iter()
                .map(|error| (error.kind, &src[error.range]))
                .collect();
            assert_eq!(errors, [(ParseErrorKind::BinaryDigits, src)], "{src:?}");
        }
    }

    #[test]
    fn decodable_binary_holding_semicolon_stays_word() {
        let parsed = parse("0x[ff;00]");
        assert!(
            !parsed
                .syntax()
                .descendants()
                .any(|node| node.kind() == SyntaxKind::Binary)
        );
        assert!(parsed.errors().is_empty());
    }
}
