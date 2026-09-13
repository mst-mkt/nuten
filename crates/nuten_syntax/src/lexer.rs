//! Lexer ported from `lex.rs` in nu-parser.

use rowan::{TextRange, TextSize};

use crate::SyntaxKind;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Token {
    pub kind: SyntaxKind,
    pub range: TextRange,
}

/// The part of nu's `lex` settings that decides where items end.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Mode {
    /// Bytes that end an item, on top of whitespace, `|`, and `;`.
    /// nu's special tokens, plus `,` where nu treats it as whitespace.
    special: &'static [u8],
}

impl Mode {
    /// Statements in the top level, `{ }` blocks, closures, and modules.
    /// Also the right-hand side of `let` and assignments.
    pub const fn block() -> Mode {
        Mode {
            special: &[],
        }
    }

    fn is_terminator(self, byte: u8) -> bool {
        matches!(byte, b' ' | b'\t' | b'\n' | b'\r' | b'|' | b';') || self.special.contains(&byte)
    }
}

/// Lexes `range` of `src` without looking at the bytes around it.
/// Token ranges are absolute in `src` and cover `range` exactly.
///
/// # Panics
///
/// Panics if `range` is outside `src` or splits a char.
pub fn lex(src: &str, range: TextRange, mode: Mode) -> Vec<Token> {
    assert!(range.end() <= TextSize::of(src), "range is outside src");
    assert!(
        src.is_char_boundary(range.start().into()) && src.is_char_boundary(range.end().into()),
        "range splits a char"
    );
    Lexer {
        src: &src.as_bytes()[..usize::from(range.end())],
        pos: usize::from(range.start()),
        mode,
    }
    .collect()
}

struct Lexer<'a> {
    src: &'a [u8],
    pos: usize,
    mode: Mode,
}

impl Iterator for Lexer<'_> {
    type Item = Token;

    fn next(&mut self) -> Option<Token> {
        let start = self.pos;
        let kind = match self.rest() {
            [] => return None,
            [b'|', b'|', ..] => self.eat(2, SyntaxKind::PipePipe),
            [b'|', ..] => self.eat(1, SyntaxKind::Pipe),
            [b';', ..] => self.eat(1, SyntaxKind::Semicolon),
            [b'\n', ..] => self.eat(1, SyntaxKind::Newline),
            [b'\r', b'\n', ..] => self.eat(2, SyntaxKind::Newline),
            [b' ' | b'\t' | b'\r', ..] => self.whitespace(),
            [b'#', ..] => self.comment(),
            _ => self.item(),
        };
        assert!(self.pos > start, "empty token at {start}");
        Some(Token {
            kind,
            range: TextRange::new(to_size(start), to_size(self.pos)),
        })
    }
}

impl<'a> Lexer<'a> {
    fn rest(&self) -> &'a [u8] {
        &self.src[self.pos..]
    }

    fn peek(&self) -> Option<u8> {
        self.rest().first().copied()
    }

    fn at_newline(&self) -> bool {
        matches!(self.rest(), [b'\n', ..] | [b'\r', b'\n', ..])
    }

    fn eat(&mut self, len: usize, kind: SyntaxKind) -> SyntaxKind {
        self.pos += len;
        kind
    }

    fn whitespace(&mut self) -> SyntaxKind {
        while matches!(self.peek(), Some(b' ' | b'\t' | b'\r')) && !self.at_newline() {
            self.pos += 1;
        }
        SyntaxKind::Whitespace
    }

    fn comment(&mut self) -> SyntaxKind {
        while self.peek().is_some() && !self.at_newline() {
            self.pos += 1;
        }
        SyntaxKind::Comment
    }

    fn item(&mut self) -> SyntaxKind {
        let len = self
            .rest()
            .iter()
            .take_while(|&&b| !self.mode.is_terminator(b))
            .count();
        self.pos += len;
        SyntaxKind::Word
    }
}

fn to_size(offset: usize) -> TextSize {
    TextSize::try_from(offset).expect("offset is within a TextRange")
}

#[cfg(test)]
mod tests {
    use rowan::{TextRange, TextSize};

    use super::{Mode, lex};
    use crate::SyntaxKind::{self, Comment, Newline, Pipe, PipePipe, Semicolon, Whitespace, Word};

    fn lexed(src: &str) -> Vec<(SyntaxKind, &str)> {
        lex(src, TextRange::up_to(TextSize::of(src)), Mode::block())
            .iter()
            .map(|token| (token.kind, &src[token.range]))
            .collect()
    }

    fn lexed_ranges(src: &str, range: TextRange) -> Vec<(SyntaxKind, TextRange)> {
        lex(src, range, Mode::block())
            .iter()
            .map(|token| (token.kind, token.range))
            .collect()
    }

    /// The split nu reports, which drops whitespace.
    fn split(src: &str) -> Vec<&str> {
        lexed(src)
            .into_iter()
            .filter(|(kind, _)| *kind != Whitespace)
            .map(|(_, text)| text)
            .collect()
    }

    fn range(start: u32, end: u32) -> TextRange {
        TextRange::new(start.into(), end.into())
    }

    #[test]
    fn empty() {
        assert!(lexed("").is_empty());
    }

    #[test]
    fn only_trivia() {
        assert_eq!(lexed(" \t"), [(Whitespace, " \t")]);
        assert_eq!(lexed("\n"), [(Newline, "\n")]);
        assert_eq!(lexed("#"), [(Comment, "#")]);
    }

    #[test]
    fn separators() {
        assert_eq!(
            lexed("ls | get name;a|b || c\n"),
            [
                (Word, "ls"),
                (Whitespace, " "),
                (Pipe, "|"),
                (Whitespace, " "),
                (Word, "get"),
                (Whitespace, " "),
                (Word, "name"),
                (Semicolon, ";"),
                (Word, "a"),
                (Pipe, "|"),
                (Word, "b"),
                (Whitespace, " "),
                (PipePipe, "||"),
                (Whitespace, " "),
                (Word, "c"),
                (Newline, "\n"),
            ]
        );
    }

    #[test]
    fn pipes_pair_greedily() {
        assert_eq!(lexed("|||"), [(PipePipe, "||"), (Pipe, "|")]);
        assert_eq!(lexed("|;|"), [(Pipe, "|"), (Semicolon, ";"), (Pipe, "|")]);
    }

    #[test]
    fn comment_tokens_start_only_at_token_start() {
        assert_eq!(lexed("foo#bar"), [(Word, "foo#bar")]);
        assert_eq!(
            lexed("foo #bar"),
            [(Word, "foo"), (Whitespace, " "), (Comment, "#bar")]
        );
        assert_eq!(lexed("a|#b"), [(Word, "a"), (Pipe, "|"), (Comment, "#b")]);
        assert_eq!(lexed("\t#c"), [(Whitespace, "\t"), (Comment, "#c")]);
    }

    #[test]
    fn comment_runs_to_end_of_line() {
        assert_eq!(
            lexed("# a | b ; c\n#d"),
            [(Comment, "# a | b ; c"), (Newline, "\n"), (Comment, "#d")]
        );
    }

    #[test]
    fn block_mode_has_no_special_bytes() {
        assert_eq!(lexed("a:b,c=d.e?f!"), [(Word, "a:b,c=d.e?f!")]);
        assert_eq!(lexed("--foo=bar"), [(Word, "--foo=bar")]);
    }

    #[test]
    fn non_ascii_bytes_belong_to_words() {
        assert_eq!(
            lexed("日本 語\t✓"),
            [
                (Word, "日本"),
                (Whitespace, " "),
                (Word, "語"),
                (Whitespace, "\t"),
                (Word, "✓"),
            ]
        );
        assert_eq!(lexed("#日本\n"), [(Comment, "#日本"), (Newline, "\n")]);
        assert_eq!(lexed("|日;"), [(Pipe, "|"), (Word, "日"), (Semicolon, ";")]);
        assert_eq!(lexed("a\u{85}b\u{a0}c"), [(Word, "a\u{85}b\u{a0}c")]);
    }

    /// How nu 0.115.1 splits each input.
    const NU_SPLITS: &[(&str, &[&str])] = &[
        ("foo#bar", &["foo#bar"]),
        ("foo #bar", &["foo", "#bar"]),
        ("echo a:b", &["echo", "a:b"]),
        ("echo a,b", &["echo", "a,b"]),
        ("echo --foo=bar", &["echo", "--foo=bar"]),
        (r"echo a\ b", &["echo", r"a\", "b"]),
        ("echo a|b", &["echo", "a", "|", "b"]),
        ("echo a;b", &["echo", "a", ";", "b"]),
        ("echo x || y", &["echo", "x", "||", "y"]),
        ("echo o> f", &["echo", "o>", "f"]),
        ("echo a\tb", &["echo", "a", "b"]),
        ("$x = 1", &["$x", "=", "1"]),
        ("echo a += 1", &["echo", "a", "+=", "1"]),
        (
            "@attr\ndef f [] {}",
            &["@attr", "\n", "def", "f", "[]", "{}"],
        ),
    ];

    #[test]
    fn splits_match_nu() {
        for &(src, expected) in NU_SPLITS {
            assert_eq!(split(src), expected, "{src:?}");
        }
    }

    // Deliberate differences from nu.

    #[test]
    fn crlf_is_one_newline() {
        assert_eq!(lexed("\r\n"), [(Newline, "\r\n")]);
        assert_eq!(
            lexed("a\r\nb"),
            [(Word, "a"), (Newline, "\r\n"), (Word, "b")]
        );
        assert_eq!(
            lexed("a \r\nb"),
            [
                (Word, "a"),
                (Whitespace, " "),
                (Newline, "\r\n"),
                (Word, "b")
            ]
        );
        assert_eq!(lexed("\r\r\n"), [(Whitespace, "\r"), (Newline, "\r\n")]);
    }

    #[test]
    fn lone_cr_is_whitespace() {
        assert_eq!(lexed("\r"), [(Whitespace, "\r")]);
        assert_eq!(
            lexed("a \r\tb"),
            [(Word, "a"), (Whitespace, " \r\t"), (Word, "b")]
        );
        assert_eq!(
            lexed("a\rb"),
            [(Word, "a"), (Whitespace, "\r"), (Word, "b")]
        );
    }

    #[test]
    fn comment_keeps_cr_except_before_lf() {
        assert_eq!(lexed("# c\rd"), [(Comment, "# c\rd")]);
        assert_eq!(lexed("# c\r"), [(Comment, "# c\r")]);
        assert_eq!(
            lexed("# c\r\nd"),
            [(Comment, "# c"), (Newline, "\r\n"), (Word, "d")]
        );
        assert_eq!(lexed("#\r\r\n"), [(Comment, "#\r"), (Newline, "\r\n")]);
    }

    #[test]
    fn leading_pipe_is_not_rewritten() {
        assert_eq!(
            lexed("a\n| b"),
            [
                (Word, "a"),
                (Newline, "\n"),
                (Pipe, "|"),
                (Whitespace, " "),
                (Word, "b"),
            ]
        );
    }

    #[test]
    fn range_is_absolute() {
        assert_eq!(
            lexed_ranges("ab cd ef", range(3, 6)),
            [(Word, range(3, 5)), (Whitespace, range(5, 6))]
        );
    }

    #[test]
    fn range_cuts_through_tokens() {
        assert_eq!(lexed_ranges("abcd", range(1, 3)), [(Word, range(1, 3))]);
        assert_eq!(
            lexed_ranges("a\r\n", range(0, 2)),
            [(Word, range(0, 1)), (Whitespace, range(1, 2))]
        );
        assert!(lexed_ranges("abcd", range(2, 2)).is_empty());
    }

    #[test]
    #[should_panic(expected = "range splits a char")]
    fn range_must_be_on_char_boundaries() {
        lex("日", range(0, 1), Mode::block());
    }

    #[test]
    #[should_panic(expected = "range is outside src")]
    fn range_must_be_within_src() {
        lex("ab", range(1, 3), Mode::block());
    }

    fn check_invariants(src: &str) {
        let tokens = lex(src, TextRange::up_to(TextSize::of(src)), Mode::block());

        let mut end = TextSize::new(0);
        for token in &tokens {
            assert_eq!(token.range.start(), end, "gap or overlap in {src:?}");
            assert!(!token.range.is_empty(), "empty token in {src:?}");
            let text = &src[token.range];
            assert!(
                well_formed(token.kind, text),
                "malformed {:?} {text:?} in {src:?}",
                token.kind
            );
            end = token.range.end();
        }
        assert_eq!(end, TextSize::of(src), "tokens end early in {src:?}");

        for pair in tokens.windows(2) {
            let (left, right) = (pair[0], pair[1]);
            assert!(
                left.kind != Whitespace || right.kind != Whitespace,
                "adjacent whitespace in {src:?}"
            );
            assert!(
                !(src[left.range].ends_with('\r') && src[right.range].starts_with('\n')),
                "split \\r\\n in {src:?}"
            );
        }
    }

    fn well_formed(kind: SyntaxKind, text: &str) -> bool {
        match kind {
            Whitespace => text.bytes().all(|b| matches!(b, b' ' | b'\t' | b'\r')),
            Newline => text == "\n" || text == "\r\n",
            Comment => text.starts_with('#') && !text.contains('\n'),
            Pipe => text == "|",
            PipePipe => text == "||",
            Semicolon => text == ";",
            Word => !text.starts_with('#') && !text.contains([' ', '\t', '\n', '\r', '|', ';']),
            // Kinds the lexer does not emit yet.
            _ => false,
        }
    }

    /// A sample of each byte class the lexer tells apart.
    const ALPHABET: &[&str] = &["a", "|", ";", "#", "\n", "\r", "\t", " ", "日"];

    fn every_input(len: usize) -> Vec<String> {
        match len {
            0 => vec![String::new()],
            _ => every_input(len - 1)
                .iter()
                .flat_map(|src| ALPHABET.iter().map(move |ch| format!("{src}{ch}")))
                .collect(),
        }
    }

    #[test]
    fn invariants_hold_for_all_short_inputs() {
        (0..=5)
            .flat_map(every_input)
            .for_each(|src| check_invariants(&src));
    }
}
