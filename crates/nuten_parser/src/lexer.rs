//! Lexer ported from `lex.rs` in nu-parser.

use std::{fmt, ops::Range};

use nuten_syntax::SyntaxKind;
use rowan::{TextRange, TextSize};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Token {
    pub kind: SyntaxKind,
    pub range: TextRange,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Lexed {
    pub tokens: Vec<Token>,
    /// One error per `Error` token, in the same order.
    pub errors: Vec<LexError>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LexError {
    pub kind: LexErrorKind,
    /// Always inside its `Error` token.
    pub range: TextRange,
}

impl LexError {
    fn new(kind: LexErrorKind, range: Range<usize>) -> LexError {
        LexError {
            kind,
            range: text_range(range),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LexErrorKind {
    /// A quote or bracket that never closes.
    /// The range covers the innermost opener, such as `[`, `$"`, or `r#'`,
    /// but in interpolation the outermost unclosed `(`.
    Unclosed,
    /// A closer that does not match the innermost opener,
    /// other than `]` with nothing open.
    UnmatchedCloser,
    /// `r#` not followed by `'`.
    /// The range covers `r` and the `#`s.
    MissingRawQuote,
}

/// One line.
impl fmt::Display for LexErrorKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            LexErrorKind::Unclosed => "unclosed quote or bracket",
            LexErrorKind::UnmatchedCloser => "unmatched closing bracket",
            LexErrorKind::MissingRawQuote => "`r#` with no `'` after it",
        })
    }
}

/// The part of nu's `lex` settings that decides where items end.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Mode {
    /// Bytes that end an item, on top of whitespace, `|`, and `;`, each with
    /// the kind of the token it makes on its own at the start of an item.
    /// nu's special tokens, plus `,` where nu treats it as whitespace.
    /// All ASCII, and none of the bytes `next` handles before `item`.
    special: &'static [(u8, SyntaxKind)],
}

impl Mode {
    pub const ALL: &[Mode] = &[Mode::block(), Mode::list()];

    /// Statements in the top level, `{ }` blocks, closures, and modules.
    /// Also the right-hand side of `let` and assignments.
    pub const fn block() -> Mode {
        Mode {
            special: &[],
        }
    }

    /// Lists, tables, and list patterns.
    pub const fn list() -> Mode {
        Mode {
            special: &[(b',', SyntaxKind::Comma)],
        }
    }

    fn is_terminator(self, byte: u8) -> bool {
        matches!(byte, b' ' | b'\t' | b'\n' | b'\r' | b'|' | b';')
            || self.special_kind(byte).is_some()
    }

    fn special_kind(self, byte: u8) -> Option<SyntaxKind> {
        self.special
            .iter()
            .find_map(|&(special, kind)| (special == byte).then_some(kind))
    }
}

/// The special bytes, as in `Mode(",")`.
impl fmt::Debug for Mode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let Mode {
            special,
        } = self;
        let bytes: String = special.iter().map(|&(byte, _)| char::from(byte)).collect();
        f.debug_tuple("Mode").field(&bytes).finish()
    }
}

/// Lexes `range` of `src` without looking at the bytes around it.
/// All ranges are absolute in `src`, and the tokens cover `range` exactly.
///
/// # Panics
///
/// Panics if `range` is outside `src` or splits a char.
pub fn lex(src: &str, range: TextRange, mode: Mode) -> Lexed {
    assert!(range.end() <= TextSize::of(src), "range is outside src");
    assert!(
        src.is_char_boundary(range.start().into()) && src.is_char_boundary(range.end().into()),
        "range splits a char"
    );
    let mut lexer = Lexer {
        src: &src.as_bytes()[..usize::from(range.end())],
        pos: usize::from(range.start()),
        mode,
        errors: Vec::new(),
    };
    Lexed {
        tokens: lexer.by_ref().collect(),
        errors: lexer.errors,
    }
}

struct Lexer<'a> {
    src: &'a [u8],
    pos: usize,
    mode: Mode,
    errors: Vec<LexError>,
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
            _ => self.item().unwrap_or_else(|error| {
                self.errors.push(error);
                SyntaxKind::Error
            }),
        };
        assert!(self.pos > start, "empty token at {start}");
        Some(Token {
            kind,
            range: text_range(start..self.pos),
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

    /// Runs to a terminator outside brackets and strings.
    /// A redirection also takes the `|` right after it.
    /// On error, the item ends where nu finds it.
    fn item(&mut self) -> Result<SyntaxKind, LexError> {
        if let Some(kind) = self.peek().and_then(|byte| self.mode.special_kind(byte)) {
            return Ok(self.eat(1, kind));
        }
        let start = self.pos;
        let mut closers = Vec::new();
        // A string that opens at `start`, with its kind and end.
        let mut leading_string = None;

        while let Some(byte) = self.peek()
            && !(closers.is_empty() && self.mode.is_terminator(byte))
        {
            let at = self.pos;
            let prev = self.src[start..at].last().copied();
            self.pos += 1;
            match byte {
                b'#' if prev.is_none_or(|b| char::from(b).is_whitespace()) => {
                    self.comment_in_item(closers.is_empty());
                },
                b'\'' | b'"' | b'`' => {
                    let interpolated = byte != b'`' && prev == Some(b'$');
                    let string_start = at - usize::from(interpolated);
                    self.string(string_start, byte, interpolated)?;
                    if string_start == start {
                        let kind = if interpolated {
                            SyntaxKind::InterpString
                        } else {
                            SyntaxKind::String
                        };
                        leading_string = Some((kind, self.pos));
                    }
                },
                b'r' if self.peek() == Some(b'#') => {
                    self.raw_string(at)?;
                    if at == start {
                        leading_string = Some((SyntaxKind::RawString, self.pos));
                    }
                },
                b'[' => closers.push((b']', at)),
                b'{' => closers.push((b'}', at)),
                b'(' => closers.push((b')', at)),
                b']' | b'}' | b')' if closers.last().is_some_and(|&(closer, _)| closer == byte) => {
                    closers.pop();
                },
                b']' if closers.is_empty() => {},
                b']' | b'}' | b')' => {
                    return Err(LexError::new(LexErrorKind::UnmatchedCloser, at..at + 1));
                },
                _ => {},
            }
        }

        if let Some(&(_, at)) = closers.last() {
            return Err(LexError::new(LexErrorKind::Unclosed, at..at + 1));
        }
        if self.peek() == Some(b'|') && is_redirection(&self.src[start..self.pos]) {
            self.pos += 1;
        }
        Ok(match leading_string {
            Some((kind, end)) if end == self.pos => kind,
            _ => operator_kind(&self.src[start..self.pos]).unwrap_or(SyntaxKind::Word),
        })
    }

    fn comment_in_item(&mut self, top_level: bool) {
        let len = self
            .rest()
            .iter()
            .take_while(|&&b| {
                if top_level {
                    !self.mode.is_terminator(b)
                } else {
                    !matches!(b, b'\n' | b'\r')
                }
            })
            .count();
        self.pos += len;
    }

    /// Consumes a string after its opening quote.
    /// The opener starts at `start`, on the `$` if interpolated.
    /// In an interpolated string, `(` opens a subexpression.
    /// Quotes and parens nest inside it.
    fn string(&mut self, start: usize, quote: u8, interpolated: bool) -> Result<(), LexError> {
        let opener = start..self.pos;
        let mut closers = vec![quote];
        let mut outer_paren = None;
        while let Some(&closer) = closers.last() {
            match (closer, self.rest()) {
                (_, []) => break,
                // nu ends the item before a final `\` in the outermost string.
                (b'"', [b'\\']) if closers.len() == 1 => break,
                (b'"', [b'\\', _, ..]) => self.pos += 1,
                (b')', &[inner @ (b'\'' | b'"' | b'`'), ..]) => closers.push(inner),
                (b')', [b'(', ..]) => closers.push(b')'),
                (_, &[byte, ..]) if byte == closer => {
                    closers.pop();
                },
                (_, [b'(', ..]) if interpolated && closers.len() == 1 => {
                    closers.push(b')');
                    outer_paren = Some(self.pos);
                },
                _ => {},
            }
            self.pos += 1;
        }
        let range = match (closers.as_slice(), outer_paren) {
            ([], _) => return Ok(()),
            ([_, _, ..], Some(at)) => at..at + 1,
            _ => opener,
        };
        Err(LexError::new(LexErrorKind::Unclosed, range))
    }

    /// Consumes a raw string after its `r`, which is at `start`.
    fn raw_string(&mut self, start: usize) -> Result<(), LexError> {
        let hashes = self.rest().iter().take_while(|&&b| b == b'#').count();
        self.pos += hashes;
        if self.peek() != Some(b'\'') {
            return Err(LexError::new(
                LexErrorKind::MissingRawQuote,
                start..self.pos,
            ));
        }
        let opener = start..self.pos + 1;
        // Like nu, search from the opening quote, so `r#'#` is complete.
        let closer = format!("'{}", "#".repeat(hashes));
        let Some(offset) = self
            .rest()
            .windows(closer.len())
            .position(|window| window == closer.as_bytes())
        else {
            self.pos = self.src.len();
            return Err(LexError::new(LexErrorKind::Unclosed, opener));
        };
        self.pos += offset + closer.len();
        Ok(())
    }
}

fn operator_kind(text: &[u8]) -> Option<SyntaxKind> {
    let kind = match text {
        b"=" | b"+=" | b"++=" | b"-=" | b"*=" | b"/=" => SyntaxKind::Assign,
        b"o>" | b"out>" => SyntaxKind::OutGt,
        b"o>>" | b"out>>" => SyntaxKind::OutGtGt,
        b"e>" | b"err>" => SyntaxKind::ErrGt,
        b"e>>" | b"err>>" => SyntaxKind::ErrGtGt,
        b"o+e>" | b"e+o>" | b"out+err>" | b"err+out>" => SyntaxKind::OutErrGt,
        b"o+e>>" | b"e+o>>" | b"out+err>>" | b"err+out>>" => SyntaxKind::OutErrGtGt,
        b"e>|" | b"err>|" => SyntaxKind::ErrGtPipe,
        b"o+e>|" | b"e+o>|" | b"out+err>|" | b"err+out>|" => SyntaxKind::OutErrGtPipe,
        _ => return None,
    };
    Some(kind)
}

/// Whether `|` right after `text` joins it, as in `e>|`.
fn is_redirection(text: &[u8]) -> bool {
    matches!(
        operator_kind(text),
        Some(SyntaxKind::OutGt | SyntaxKind::ErrGt | SyntaxKind::OutErrGt)
    )
}

fn text_range(range: Range<usize>) -> TextRange {
    let size = |offset: usize| TextSize::try_from(offset).expect("offset is within a TextRange");
    TextRange::new(size(range.start), size(range.end))
}

#[cfg(test)]
mod tests {
    use nuten_syntax::SyntaxKind::{
        self, Assign, Comma, Comment, ErrGt, ErrGtGt, ErrGtPipe, Error, InterpString, Newline,
        OutErrGt, OutErrGtGt, OutErrGtPipe, OutGt, OutGtGt, Pipe, PipePipe, RawString, Semicolon,
        Whitespace, Word,
    };
    use rowan::{TextRange, TextSize};

    use super::{
        LexError,
        LexErrorKind::{self, MissingRawQuote, Unclosed, UnmatchedCloser},
        Lexed, Mode, Token, lex,
    };
    use crate::test_support::short_inputs;

    fn lex_all(src: &str) -> Lexed {
        lex_all_in(src, Mode::block())
    }

    fn lex_all_in(src: &str, mode: Mode) -> Lexed {
        lex(src, TextRange::up_to(TextSize::of(src)), mode)
    }

    fn lexed(src: &str) -> Vec<(SyntaxKind, &str)> {
        lexed_in(src, Mode::block())
    }

    fn lexed_in(src: &str, mode: Mode) -> Vec<(SyntaxKind, &str)> {
        lex_all_in(src, mode)
            .tokens
            .iter()
            .map(|token| (token.kind, &src[token.range]))
            .collect()
    }

    fn lexed_ranges(src: &str, range: TextRange) -> Vec<(SyntaxKind, TextRange)> {
        lex(src, range, Mode::block())
            .tokens
            .iter()
            .map(|token| (token.kind, token.range))
            .collect()
    }

    fn error_of(src: &str) -> (LexErrorKind, &str) {
        let errors = lex_all(src).errors;
        assert_eq!(
            errors.len(),
            1,
            "{src:?} does not have one error: {errors:?}"
        );
        (errors[0].kind, &src[errors[0].range])
    }

    /// The split nu reports, which drops whitespace.
    fn split(src: &str) -> Vec<&str> {
        split_skipping(src, Mode::block(), &[Whitespace])
    }

    /// The split nu reports inside a list, where it reads `,` and line breaks
    /// as whitespace too.
    fn split_list(src: &str) -> Vec<&str> {
        split_skipping(src, Mode::list(), &[Whitespace, Newline, Comma])
    }

    fn split_skipping<'a>(src: &'a str, mode: Mode, skipped: &[SyntaxKind]) -> Vec<&'a str> {
        lexed_in(src, mode)
            .into_iter()
            .filter(|(kind, _)| !skipped.contains(kind))
            .map(|(_, text)| text)
            .collect()
    }

    fn kind_of(src: &str) -> SyntaxKind {
        let tokens = lexed(src);
        assert_eq!(tokens.len(), 1, "{src:?} is not one token: {tokens:?}");
        tokens[0].0
    }

    fn assert_one_item(item: &str) {
        assert_eq!(split(&format!("{item} z")), [item, "z"], "{item:?}");
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
        assert_eq!(lexed(",a"), [(Word, ",a")]);
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

    #[test]
    fn terminators_do_not_end_nested_items() {
        for item in ["[a\n| b; c]", "{a (b | c)}", "'a | b;c'"] {
            assert_one_item(item);
        }
    }

    #[test]
    fn strings_inside_brackets_hide_closers() {
        for item in [r#"[a "]" b]"#, "[r#'a']'#]"] {
            assert_one_item(item);
        }
    }

    #[test]
    fn backslash_escapes_only_in_double_quotes() {
        assert_eq!(kind_of(r#""a\"b""#), SyntaxKind::String);
        assert_eq!(kind_of(r"'a\'"), SyntaxKind::String);
        assert_eq!(kind_of(r"`a\`"), SyntaxKind::String);
    }

    #[test]
    fn interpolation_nests_quotes_and_parens() {
        for item in [
            r#"$"a(")")b""#,
            r#"$"((1) + ")")""#,
            r#"$"('"')""#,
            r#"$"(`"`)""#,
            r#"$"('(')""#,
            r#"$"("\"")""#,
            r#"$"(\)""#,
            "$'a(')')b'",
            r#"x$"(")")""#,
        ] {
            assert_one_item(item);
        }
    }

    #[test]
    fn other_parens_in_strings_are_plain() {
        for item in [r#""a(""#, r#"$"a\(""#, "$`a(`"] {
            assert_one_item(item);
        }
    }

    #[test]
    fn comment_inside_brackets_hides_delimiters() {
        for item in ["[a # ] b\n]", "[a # ] b\r]", "[a # b#] c\n]"] {
            assert_one_item(item);
        }
        assert_eq!(split("[a#] b]"), ["[a#]", "b]"]);
    }

    /// nu reads the byte before `#` as Latin-1.
    /// `à` (`C3 A0`) ends in whitespace, so a comment starts inside the item.
    #[test]
    fn hash_after_latin1_whitespace_byte_starts_comment() {
        assert_eq!(split(r#"à#"b c"#), [r#"à#"b"#, "c"]);
        assert_eq!(split(r#"a#"b c""#), [r#"a#"b c""#]);
    }

    #[test]
    fn raw_string_closes_at_the_first_closer() {
        assert_eq!(kind_of("r##'a '# b'##"), RawString);
        assert_eq!(split("r#'a'# r#'b'#"), ["r#'a'#", "r#'b'#"]);
        assert_eq!(kind_of("r#'#"), RawString);
    }

    #[test]
    fn pipe_joins_a_redirection() {
        assert_eq!(split("e>|cat"), ["e>|", "cat"]);
        assert_eq!(split("o+e>|cat"), ["o+e>|", "cat"]);
        assert_eq!(split("e>>|cat"), ["e>>", "|", "cat"]);
        assert_eq!(split("xe>|cat"), ["xe>", "|", "cat"]);
    }

    #[test]
    fn single_literal_items_get_literal_kinds() {
        assert_eq!(kind_of("'a b'"), SyntaxKind::String);
        assert_eq!(kind_of("`a`"), SyntaxKind::String);
        assert_eq!(kind_of(r#"$"a(1 + ")")""#), InterpString);
        assert_eq!(kind_of("$'a(1)'"), InterpString);
        assert_eq!(kind_of("r#'a'#"), RawString);
    }

    #[test]
    fn other_items_are_words() {
        for item in [
            r#"a"b c"d"#,
            r#""a"b"#,
            r#""a"'b'"#,
            r#"$"a"b"#,
            r#""a"r#'b'#"#,
            r#"r#'a'#"b""#,
            "r#'a'#b",
            "$`a`",
            r#"r"a""#,
            "[a b]",
            "(1 + 2)",
        ] {
            assert_eq!(kind_of(item), Word, "{item:?}");
        }
    }

    /// Every spelling of each operator kind.
    const OPERATORS: &[(SyntaxKind, &[&str])] = &[
        (Assign, &["=", "+=", "++=", "-=", "*=", "/="]),
        (OutGt, &["o>", "out>"]),
        (OutGtGt, &["o>>", "out>>"]),
        (ErrGt, &["e>", "err>"]),
        (ErrGtGt, &["e>>", "err>>"]),
        (OutErrGt, &["o+e>", "e+o>", "out+err>", "err+out>"]),
        (OutErrGtGt, &["o+e>>", "e+o>>", "out+err>>", "err+out>>"]),
        (ErrGtPipe, &["e>|", "err>|"]),
        (OutErrGtPipe, &["o+e>|", "e+o>|", "out+err>|", "err+out>|"]),
    ];

    #[test]
    fn operators_get_their_kinds() {
        for &(expected, spellings) in OPERATORS {
            for &text in spellings {
                assert_eq!(kind_of(text), expected, "{text:?}");
            }
        }
        // nu reports these, and nuten leaves them to the parser.
        for text in ["o>|", "out>|", "&&", "2>", "2>&1"] {
            assert_eq!(kind_of(text), Word, "{text:?}");
        }
    }

    #[test]
    fn unclosed_item_runs_to_the_end() {
        assert_eq!(
            lexed(r#"a "b c"#),
            [(Word, "a"), (Whitespace, " "), (Error, r#""b c"#)]
        );
        assert_eq!(kind_of("[a\n(b c"), Error);
        assert_eq!(kind_of(r#"$"(a" b"#), Error);
        assert_eq!(kind_of("r#'a' b"), Error);
    }

    #[test]
    fn error_ends_the_item_where_found() {
        assert_eq!(lexed("a}b"), [(Error, "a}"), (Word, "b")]);
        assert_eq!(lexed("a)b"), [(Error, "a)"), (Word, "b")]);
        assert_eq!(lexed("[a)b]"), [(Error, "[a)"), (Word, "b]")]);
        assert_eq!(lexed("{[}]"), [(Error, "{[}"), (Word, "]")]);
        assert_eq!(lexed("r#x"), [(Error, "r#"), (Word, "x")]);
    }

    #[test]
    fn final_backslash_splits_off_only_from_the_outermost_string() {
        assert_eq!(lexed(r#""a\"#), [(Error, r#""a"#), (Word, r"\")]);
        assert_eq!(lexed(r#"$"("\"#), [(Error, r#"$"("\"#)]);
    }

    #[test]
    fn unclosed_points_at_the_innermost_opener() {
        for (src, opener) in [
            (r#""a b"#, "\""),
            ("'a", "'"),
            ("$`a", "`"),
            ("a[b", "["),
            (r#"{a "b""#, "{"),
            ("[a\n(b c", "("),
            (r#"[a "b"#, "\""),
            (r#""a\"#, "\""),
            (r#"$"(a)b"#, r#"$""#),
        ] {
            assert_eq!(error_of(src), (Unclosed, opener), "{src:?}");
        }
    }

    #[test]
    fn unclosed_interpolation_points_at_the_outermost_paren() {
        for (src, paren) in [
            ("$'(a", 2),
            (r#"$"(a" b"#, 2),
            (r#"$"a(b(c"#, 3),
            (r#"$"(a)(b"#, 5),
        ] {
            assert_eq!(
                lex_all(src).errors,
                [LexError {
                    kind: Unclosed,
                    range: range(paren, paren + 1),
                }],
                "{src:?}"
            );
        }
    }

    #[test]
    fn unmatched_closer_points_at_the_closer() {
        for (src, closer) in [
            ("a}b", "}"),
            ("a)b", ")"),
            ("[a)b]", ")"),
            ("{[}]", "}"),
            ("(a]", "]"),
        ] {
            assert_eq!(error_of(src), (UnmatchedCloser, closer), "{src:?}");
        }
    }

    /// How nu 0.115.1 splits each input.
    const NU_SPLITS: &[(&str, &[&str])] = &[
        ("foo#bar", &["foo#bar"]),
        ("foo #bar", &["foo", "#bar"]),
        ("echo [a b]#c", &["echo", "[a b]#c"]),
        (r##"echo "a"#b"##, &["echo", r##""a"#b"##]),
        ("echo a:b", &["echo", "a:b"]),
        ("echo a,b", &["echo", "a,b"]),
        ("echo --foo=bar", &["echo", "--foo=bar"]),
        (r#"echo a"b"c"#, &["echo", r#"a"b"c"#]),
        (r#"echo ab"c d"e"#, &["echo", r#"ab"c d"e"#]),
        (r#"echo "a"b"#, &["echo", r#""a"b"#]),
        (r"echo 'a\'b", &["echo", r"'a\'b"]),
        (r#"echo "a\"#, &["echo", r#""a"#, r"\"]),
        (r"echo a\ b", &["echo", r"a\", "b"]),
        (r#"echo $"x(1 + 2)y""#, &["echo", r#"$"x(1 + 2)y""#]),
        (r#"echo $"x\(1)y""#, &["echo", r#"$"x\(1)y""#]),
        ("echo $'x(1)y'", &["echo", "$'x(1)y'"]),
        ("echo $`x(1)y`", &["echo", "$`x(1)y`"]),
        ("echo r#'a'b'#", &["echo", "r#'a'b'#"]),
        ("echo r##'a'#b'##", &["echo", "r##'a'#b'##"]),
        ("echo r#'a\nb'#", &["echo", "r#'a\nb'#"]),
        ("echo r#x", &["echo", "r#", "x"]),
        ("echo r#'#", &["echo", "r#'#"]),
        ("echo xr#'a'#", &["echo", "xr#'a'#"]),
        ("echo a|b", &["echo", "a", "|", "b"]),
        ("echo a;b", &["echo", "a", ";", "b"]),
        ("echo x || y", &["echo", "x", "||", "y"]),
        ("echo a[b]", &["echo", "a[b]"]),
        ("echo a]b", &["echo", "a]b"]),
        ("echo a}b", &["echo", "a}", "b"]),
        ("echo [a]b", &["echo", "[a]b"]),
        ("echo (1)x", &["echo", "(1)x"]),
        ("echo {a:1}b", &["echo", "{a:1}b"]),
        ("echo o> f", &["echo", "o>", "f"]),
        ("echo e>| cat", &["echo", "e>|", "cat"]),
        ("echo a\tb", &["echo", "a", "b"]),
        ("$x = 1", &["$x", "=", "1"]),
        ("echo a += 1", &["echo", "a", "+=", "1"]),
        (
            "@attr\ndef f [] {}",
            &["@attr", "\n", "def", "f", "[]", "{}"],
        ),
    ];

    /// The rows where nu reports a lexing error.
    const NU_ERRORS: &[(&str, LexErrorKind)] = &[
        (r#"echo "a\"#, Unclosed),
        ("echo r#x", MissingRawQuote),
        ("echo a}b", UnmatchedCloser),
    ];

    #[test]
    fn splits_match_nu() {
        for &(src, expected) in NU_SPLITS {
            assert_eq!(split(src), expected, "{src:?}");
        }
    }

    /// How nu 0.115.1 splits the inside of a list.
    const NU_LIST_SPLITS: &[(&str, &[&str])] = &[
        ("a,b", &["a", "b"]),
        ("a,,b", &["a", "b"]),
        ("a , b", &["a", "b"]),
        ("a,", &["a"]),
        (",a", &["a"]),
        (" , ", &[]),
        ("a,\nb", &["a", "b"]),
        ("a\r,b", &["a", "b"]),
        ("a,|b", &["a", "|", "b"]),
        ("a=b", &["a=b"]),
        ("a:b", &["a:b"]),
        ("--x=1", &["--x=1"]),
        (r#""a,b""#, &[r#""a,b""#]),
        ("a[b,c]d", &["a[b,c]d"]),
        ("a[b,#c] d", &["a[b,#c]", "d"]),
        ("[a #,] b\n],c", &["[a #,] b\n]", "c"]),
        ("a#b,c", &["a#b", "c"]),
        ("à#b,c", &["à#b", "c"]),
        ("a},b", &["a}", "b"]),
        ("[a,b", &["[a,b"]),
    ];

    #[test]
    fn list_splits_match_nu() {
        for &(src, expected) in NU_LIST_SPLITS {
            assert_eq!(split_list(src), expected, "{src:?}");
        }
    }

    #[test]
    fn special_bytes_are_tokens_at_the_start_of_an_item() {
        assert_eq!(
            lexed_in(",a,,b ,", Mode::list()),
            [
                (Comma, ","),
                (Word, "a"),
                (Comma, ","),
                (Comma, ","),
                (Word, "b"),
                (Whitespace, " "),
                (Comma, ","),
            ]
        );
    }

    #[test]
    fn items_ending_at_a_special_byte_keep_their_kinds() {
        for (src, kind) in [
            (r#""a",b"#, SyntaxKind::String),
            (r#"$"(a,b)",b"#, InterpString),
            ("r#'a,b'#,b", RawString),
            ("e>,b", ErrGt),
            ("e>|,b", ErrGtPipe),
            ("=,b", Assign),
        ] {
            let tokens = lexed_in(src, Mode::list());
            assert_eq!(tokens[0].0, kind, "{src:?}");
            assert_eq!(&tokens[1..], [(Comma, ","), (Word, "b")], "{src:?}");
        }
    }

    #[test]
    fn errors_match_nu() {
        for &(src, _) in NU_SPLITS {
            let kinds: Vec<_> = lex_all(src).errors.iter().map(|error| error.kind).collect();
            let expected = NU_ERRORS
                .iter()
                .find(|&&(row, _)| row == src)
                .map(|&(_, kind)| kind);
            assert_eq!(kinds, expected.as_slice(), "{src:?}");
        }
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
        assert_eq!(
            lexed_ranges(r#""a b""#, range(0, 3)),
            [(Error, range(0, 3))]
        );
        assert!(lexed_ranges("abcd", range(2, 2)).is_empty());
    }

    #[test]
    fn bytes_before_the_range_are_ignored() {
        assert_eq!(
            lexed_ranges(r#"$"(" x"#, range(1, 6)),
            [
                (SyntaxKind::String, range(1, 4)),
                (Whitespace, range(4, 5)),
                (Word, range(5, 6)),
            ]
        );
        assert_eq!(
            lex(r#"$"a"#, range(1, 3), Mode::block()).errors,
            [LexError {
                kind: Unclosed,
                range: range(1, 2),
            }]
        );
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

    fn check_invariants(src: &str, mode: Mode) {
        let Lexed {
            tokens,
            errors,
        } = lex_all_in(src, mode);

        let mut end = TextSize::new(0);
        for token in &tokens {
            assert_eq!(
                token.range.start(),
                end,
                "{mode:?}: gap or overlap in {src:?}"
            );
            assert!(!token.range.is_empty(), "{mode:?}: empty token in {src:?}");
            let text = &src[token.range];
            assert!(
                well_formed(token.kind, text, mode),
                "{mode:?}: malformed {:?} {text:?} in {src:?}",
                token.kind
            );
            end = token.range.end();
        }
        assert_eq!(
            end,
            TextSize::of(src),
            "{mode:?}: tokens end early in {src:?}"
        );

        for pair in tokens.windows(2) {
            let (left, right) = (pair[0], pair[1]);
            assert!(
                left.kind != Whitespace || right.kind != Whitespace,
                "{mode:?}: adjacent whitespace in {src:?}"
            );
            assert!(
                !(src[left.range].ends_with('\r') && src[right.range].starts_with('\n')),
                "{mode:?}: split \\r\\n in {src:?}"
            );
        }

        let error_tokens: Vec<_> = tokens.iter().filter(|token| token.kind == Error).collect();
        assert_eq!(
            error_tokens.len(),
            errors.len(),
            "{mode:?}: errors do not match Error tokens in {src:?}"
        );
        for (&token, &error) in error_tokens.into_iter().zip(&errors) {
            assert!(
                well_placed(error, token, src),
                "{mode:?}: misplaced {error:?} in {token:?} of {src:?}"
            );
        }
    }

    fn well_placed(error: LexError, token: Token, src: &str) -> bool {
        let raw_prefix = |text: &str| {
            text.strip_prefix('r')
                .is_some_and(|hashes| !hashes.is_empty() && hashes.bytes().all(|b| b == b'#'))
        };
        let text = &src[error.range];
        let after = &src[usize::from(token.range.end())..];
        let ends_token = error.range.end() == token.range.end();
        token.range.contains_range(error.range)
            && match error.kind {
                // Runs to the end, or before a final `\` in a `"` string.
                Unclosed => {
                    (matches!(text, "[" | "{" | "(" | "'" | "\"" | "`" | "$'" | "$\"")
                        || text.strip_suffix('\'').is_some_and(raw_prefix))
                        && (after.is_empty() || after == "\\" && text.ends_with('"'))
                },
                UnmatchedCloser => matches!(text, "]" | "}" | ")") && ends_token,
                MissingRawQuote => raw_prefix(text) && ends_token,
            }
    }

    fn well_formed(kind: SyntaxKind, text: &str, mode: Mode) -> bool {
        let quoted = |text: &str| {
            text.len() >= 2 && text.starts_with(['\'', '"', '`']) && text.ends_with(&text[..1])
        };
        match kind {
            Whitespace => text.bytes().all(|b| matches!(b, b' ' | b'\t' | b'\r')),
            Newline => text == "\n" || text == "\r\n",
            Comment => text.starts_with('#') && !text.contains('\n'),
            Pipe => text == "|",
            PipePipe => text == "||",
            Semicolon => text == ";",
            SyntaxKind::String => quoted(text),
            InterpString => text
                .strip_prefix('$')
                .is_some_and(|rest| rest.starts_with(['"', '\'']) && quoted(rest)),
            RawString => text.starts_with("r#") && text.ends_with('#'),
            Word | Error => {
                !text.starts_with(['#', ' ', '\t', '\n', '\r', '|', ';'])
                    && mode.special_kind(text.as_bytes()[0]).is_none()
            },
            // Special tokens, operators, or kinds not emitted yet.
            _ => {
                (text.len() == 1 && mode.special_kind(text.as_bytes()[0]) == Some(kind))
                    || OPERATORS
                        .iter()
                        .any(|&(operator, spellings)| operator == kind && spellings.contains(&text))
            },
        }
    }

    #[test]
    fn invariants_hold_for_all_short_inputs() {
        for src in short_inputs() {
            for &mode in Mode::ALL {
                check_invariants(&src, mode);
            }
        }
    }

    mod nu_differences {
        use super::*;

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
        fn unclosed_opener_includes_its_prefix() {
            for (src, opener) in [
                (r#"$"a"#, r#"$""#),
                ("x$'a", "$'"),
                ("r#'a' b", "r#'"),
                ("r##'a'#", "r##'"),
            ] {
                assert_eq!(error_of(src), (Unclosed, opener), "{src:?}");
            }
        }

        #[test]
        fn missing_raw_quote_points_at_the_prefix() {
            assert_eq!(error_of("r#x"), (MissingRawQuote, "r#"));
            assert_eq!(error_of("xr##"), (MissingRawQuote, "r##"));
        }
    }
}
