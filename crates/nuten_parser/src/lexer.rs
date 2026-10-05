//! Lexer ported from `lex.rs` in nu-parser.

use std::{fmt, ops::Range};

use nuten_syntax::SyntaxKind;
use rowan::{TextRange, TextSize};

#[derive(Debug, Clone, Copy, Eq)]
pub(crate) struct Token {
    pub(crate) kind: SyntaxKind,
    pub(crate) range: TextRange,
    first_bracket: FirstBracket,
}

// A token put together without the lexer does not know its first bracket.
impl PartialEq for Token {
    fn eq(&self, other: &Token) -> bool {
        self.kind == other.kind && self.range == other.range
    }
}

impl Token {
    pub(crate) fn new(kind: SyntaxKind, range: TextRange) -> Token {
        Token {
            kind,
            range,
            first_bracket: FirstBracket::UNKNOWN,
        }
    }

    pub(crate) fn first_bracket_spans(self, range: TextRange) -> Option<bool> {
        let byte = |len: TextSize| {
            u8::try_from(u32::from(len))
                .ok()
                .filter(|&len| len < u8::MAX)
        };
        if byte(range.start() - self.range.start())? != self.first_bracket.from_start {
            return None;
        }
        Some(byte(self.range.end() - range.end())? == self.first_bracket.to_end)
    }
}

/// Bytes keep it within the padding of a token, which is copied a lot.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct FirstBracket {
    from_start: u8,
    to_end: u8,
}

impl FirstBracket {
    const UNKNOWN: FirstBracket = FirstBracket {
        from_start: u8::MAX,
        to_end: u8::MAX,
    };

    fn of(item: Range<usize>, bracket: Range<usize>) -> FirstBracket {
        let byte = |len: usize| u8::try_from(len).unwrap_or(u8::MAX);
        FirstBracket {
            from_start: byte(bracket.start - item.start),
            to_end: byte(item.end - bracket.end),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Lexed {
    pub(crate) tokens: Vec<Token>,
    pub(crate) errors: Vec<LexError>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct LexError {
    pub(crate) kind: LexErrorKind,
    pub(crate) range: TextRange,
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
    Unclosed,
    UnmatchedCloser,
    MissingRawQuote,
}

impl fmt::Display for LexErrorKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            LexErrorKind::Unclosed => "unclosed quote or bracket",
            LexErrorKind::UnmatchedCloser => "unmatched closing bracket",
            LexErrorKind::MissingRawQuote => "`r#` with no `'` after it",
        })
    }
}

/// Each mode is one of the ways nu calls `lex`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Mode {
    Block,
    /// Nu lexes a line break as whitespace there, so no statement ends at one.
    Paren,
    List,
    Record,
    /// Nu does not treat `:` as special in a value of a record.
    RecordValue,
    Brace,
    VarType,
    Signature,
    /// Nu lexes a line break as whitespace there, as it does a `,`.
    IoTypes,
    TypeParams,
    /// Nu lexes a line break as whitespace there, as it does a `,`.
    MatchArms,
    CellPath,
    /// Nu lexes it only to see that it holds no error before it reads the range.
    RangeFrom,
    Binary,
    /// Unlike in a record, nu lexes what follows a `:` with `:` special too.
    RecordPattern,
}

impl Mode {
    pub(crate) const ALL: &[Mode] = &[
        Mode::Block,
        Mode::Paren,
        Mode::List,
        Mode::Record,
        Mode::RecordValue,
        Mode::Brace,
        Mode::VarType,
        Mode::Signature,
        Mode::IoTypes,
        Mode::TypeParams,
        Mode::MatchArms,
        Mode::CellPath,
        Mode::RangeFrom,
        Mode::Binary,
        Mode::RecordPattern,
    ];

    /// All ASCII, and none of the bytes `next` handles before `item`.
    #[expect(
        clippy::match_same_arms,
        reason = "one arm per way nu calls lex, to check each against nu on its own"
    )]
    fn special(self) -> &'static [(u8, SyntaxKind)] {
        match self {
            Mode::Block => &[],
            Mode::Paren => &[],
            Mode::List => &[(b',', SyntaxKind::Comma)],
            Mode::Record => &[(b':', SyntaxKind::Colon), (b',', SyntaxKind::Comma)],
            Mode::RecordValue => &[(b',', SyntaxKind::Comma)],
            Mode::Brace => &[(b':', SyntaxKind::Colon)],
            Mode::VarType => &[(b',', SyntaxKind::Comma)],
            Mode::Signature => &[
                (b':', SyntaxKind::Colon),
                (b'=', SyntaxKind::Eq),
                (b',', SyntaxKind::Comma),
            ],
            Mode::IoTypes => &[(b',', SyntaxKind::Comma)],
            Mode::TypeParams => &[(b':', SyntaxKind::Colon), (b',', SyntaxKind::Comma)],
            Mode::MatchArms => &[(b',', SyntaxKind::Comma)],
            Mode::CellPath => &[
                (b'.', SyntaxKind::Dot),
                (b'?', SyntaxKind::Question),
                (b'!', SyntaxKind::Bang),
            ],
            Mode::RangeFrom => &[
                (b'.', SyntaxKind::Dot),
                (b'?', SyntaxKind::Question),
                (b'!', SyntaxKind::Bang),
            ],
            Mode::Binary => &[(b',', SyntaxKind::Comma)],
            Mode::RecordPattern => &[(b':', SyntaxKind::Colon), (b',', SyntaxKind::Comma)],
        }
    }

    /// Nu's `lex_signature` nests `<` and `>` as brackets where it reads a type.
    fn nests_angles(self) -> bool {
        matches!(
            self,
            Mode::VarType | Mode::Signature | Mode::IoTypes | Mode::TypeParams
        )
    }

    fn is_terminator(self, byte: u8) -> bool {
        matches!(byte, b' ' | b'\t' | b'\n' | b'\r' | b'|' | b';')
            || self.special_kind(byte).is_some()
    }

    pub(crate) fn skips_newline(self) -> bool {
        matches!(self, Mode::Paren | Mode::MatchArms | Mode::IoTypes)
    }

    pub(crate) fn special_kind(self, byte: u8) -> Option<SyntaxKind> {
        self.special()
            .iter()
            .find_map(|&(special, kind)| (special == byte).then_some(kind))
    }
}

/// Lexes `range` of `src` without looking at the bytes around it.
/// All ranges are absolute in `src`.
pub(crate) fn lex(src: &str, range: TextRange, mode: Mode) -> Lexed {
    let mut lexer = Lexer::new(src, range, mode);
    Lexed {
        tokens: lexer.by_ref().collect(),
        errors: lexer.errors,
    }
}

pub(crate) fn item_brackets(src: &str, range: TextRange, mode: Mode) -> Vec<TextRange> {
    let mut lexer = Lexer::new(src, range, mode);
    lexer.brackets = Some(Vec::new());
    // An item with an error still gives the brackets it closed before it.
    let _ = lexer.item();
    lexer.brackets.unwrap_or_default()
}

/// An `Error` token comes without its error.
pub(crate) fn tokens(src: &str, range: TextRange, mode: Mode) -> impl Iterator<Item = Token> {
    Lexer::new(src, range, mode)
}

struct Lexer<'a> {
    src: &'a str,
    pos: usize,
    mode: Mode,
    errors: Vec<LexError>,
    brackets: Option<Vec<TextRange>>,
}

impl Iterator for Lexer<'_> {
    type Item = Token;

    fn next(&mut self) -> Option<Token> {
        let start = self.pos;
        let unknown = |kind| (kind, FirstBracket::UNKNOWN);
        let (kind, first_bracket) = match self.rest() {
            [] => return None,
            [b'|', b'|', ..] => unknown(self.eat(2, SyntaxKind::PipePipe)),
            [b'|', ..] => unknown(self.eat(1, SyntaxKind::Pipe)),
            [b';', ..] => unknown(self.eat(1, SyntaxKind::Semicolon)),
            [b'\n', ..] => unknown(self.eat(1, SyntaxKind::Newline)),
            [b'\r', b'\n', ..] => unknown(self.eat(2, SyntaxKind::Newline)),
            [b' ' | b'\t' | b'\r', ..] => unknown(self.whitespace()),
            [b'#', ..] => unknown(self.comment()),
            _ => self.item().unwrap_or_else(|error| {
                self.errors.push(error);
                unknown(SyntaxKind::Error)
            }),
        };
        assert!(self.pos > start, "empty token at {start}");
        Some(Token {
            kind,
            range: text_range(start..self.pos),
            first_bracket,
        })
    }
}

impl<'a> Lexer<'a> {
    fn new(src: &'a str, range: TextRange, mode: Mode) -> Lexer<'a> {
        assert!(range.end() <= TextSize::of(src), "range is outside src");
        assert!(
            src.is_char_boundary(range.start().into()) && src.is_char_boundary(range.end().into()),
            "range splits a char"
        );
        Lexer {
            src: &src[..usize::from(range.end())],
            pos: usize::from(range.start()),
            mode,
            errors: Vec::new(),
            brackets: None,
        }
    }

    fn rest(&self) -> &'a [u8] {
        &self.src.as_bytes()[self.pos..]
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
        let rest = &self.src[self.pos..];
        self.pos += rest.find('\n').map_or(rest.len(), |newline| {
            newline - usize::from(rest[..newline].ends_with('\r'))
        });
        SyntaxKind::Comment
    }

    fn item(&mut self) -> Result<(SyntaxKind, FirstBracket), LexError> {
        if let Some(kind) = self.peek().and_then(|byte| self.mode.special_kind(byte)) {
            return Ok((self.eat(1, kind), FirstBracket::UNKNOWN));
        }
        let start = self.pos;
        let mut closers = Vec::new();
        let mut first_bracket = None;
        let mut leading_string = None;

        while let Some(byte) = self.peek()
            && !(closers.is_empty() && self.mode.is_terminator(byte))
        {
            let at = self.pos;
            let prev = self.src.as_bytes()[start..at].last().copied();
            self.pos += 1;
            match byte {
                b'#' if starts_comment_after(prev) => {
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
                b'<' if self.mode.nests_angles() => closers.push((b'>', at)),
                b']' | b'}' | b')' | b'>'
                    if closers.last().is_some_and(|&(closer, _)| closer == byte) =>
                {
                    if let Some((_, opened)) = closers.pop()
                        && closers.is_empty()
                    {
                        first_bracket.get_or_insert(opened..at + 1);
                        if let Some(brackets) = &mut self.brackets {
                            brackets.push(text_range(opened..at + 1));
                        }
                    }
                },
                // Nu keeps a `]` with nothing open in the word, unlike `}` and `)`.
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
        if self.peek() == Some(b'|') && is_redirection(&self.src.as_bytes()[start..self.pos]) {
            self.pos += 1;
        }
        let kind = match leading_string {
            Some((kind, end)) if end == self.pos => kind,
            _ => operator_kind(&self.src.as_bytes()[start..self.pos]).unwrap_or(SyntaxKind::Word),
        };
        let first_bracket = first_bracket.map_or(FirstBracket::UNKNOWN, |bracket| {
            FirstBracket::of(start..self.pos, bracket)
        });
        Ok((kind, first_bracket))
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

    /// Called after the opening quote, whose opener starts at `start`, on the `$` if interpolated.
    fn string(&mut self, start: usize, quote: u8, interpolated: bool) -> Result<(), LexError> {
        let opener = start..self.pos;
        // The quote stays off the stack, so that a string nesting nothing takes no allocation.
        let mut nested = Vec::new();
        let mut outer_paren = None;
        loop {
            let closer = nested.last().copied().unwrap_or(quote);
            match (closer, self.rest()) {
                (_, []) => break,
                // Nu ends the item before a final `\` in the outermost string.
                (b'"', [b'\\']) if nested.is_empty() => break,
                (b'"', [b'\\', _, ..]) => self.pos += 1,
                (b')', &[inner @ (b'\'' | b'"' | b'`'), ..]) => nested.push(inner),
                (b')', [b'(', ..]) => nested.push(b')'),
                (_, &[byte, ..]) if byte == closer => {
                    if nested.pop().is_none() {
                        self.pos += 1;
                        return Ok(());
                    }
                },
                (_, [b'(', ..]) if interpolated && nested.is_empty() => {
                    nested.push(b')');
                    outer_paren = Some(self.pos);
                },
                _ => {},
            }
            self.pos += 1;
        }
        let range = match (nested.as_slice(), outer_paren) {
            ([_, ..], Some(at)) => at..at + 1,
            _ => opener,
        };
        Err(LexError::new(LexErrorKind::Unclosed, range))
    }

    /// Called after the `r`, which is at `start`.
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

/// Nu takes it as one only after whitespace, as it does at the start of an item.
/// A byte is whitespace as the Latin-1 character it stands for, as nu checks it.
pub(crate) fn starts_comment_after(prev: Option<u8>) -> bool {
    prev.is_none_or(|byte| char::from(byte).is_whitespace())
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

/// Nu lexes these as the pipe or redirection it takes them to mean, and then reports an error.
/// They stay words here, so the grammar looks up the kind nu reads them with.
pub(crate) fn rejected_operator_kind(text: &str) -> Option<SyntaxKind> {
    match text {
        "o>|" | "out>|" | "&&" | "2>&1" => Some(SyntaxKind::Pipe),
        "2>" => Some(SyntaxKind::ErrGt),
        _ => None,
    }
}

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
        self, Assign, Colon, Comma, Comment, ErrGt, ErrGtPipe, Error, InterpString, Newline, Pipe,
        PipePipe, RawString, Semicolon, Whitespace, Word,
    };
    use rowan::{TextRange, TextSize};

    use super::{
        LexError,
        LexErrorKind::{self, MissingRawQuote, Unclosed, UnmatchedCloser},
        Lexed, Mode, item_brackets, lex, rejected_operator_kind,
    };
    use crate::{invariants::OPERATORS, test_support::range};

    fn lex_all(src: &str) -> Lexed {
        lex_all_in(src, Mode::Block)
    }

    fn lex_all_in(src: &str, mode: Mode) -> Lexed {
        lex(src, TextRange::up_to(TextSize::of(src)), mode)
    }

    fn lexed(src: &str) -> Vec<(SyntaxKind, &str)> {
        lexed_in(src, Mode::Block)
    }

    fn lexed_in(src: &str, mode: Mode) -> Vec<(SyntaxKind, &str)> {
        lex_all_in(src, mode)
            .tokens
            .iter()
            .map(|token| (token.kind, &src[token.range]))
            .collect()
    }

    fn lexed_ranges(src: &str, range: TextRange) -> Vec<(SyntaxKind, TextRange)> {
        lex(src, range, Mode::Block)
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

    fn split(src: &str) -> Vec<&str> {
        split_in(src, Mode::Block)
    }

    fn split_in(src: &str, mode: Mode) -> Vec<&str> {
        let skipped = skipped_in(mode);
        lexed_in(src, mode)
            .into_iter()
            .filter(|(kind, _)| !skipped.contains(kind))
            .map(|(_, text)| text)
            .collect()
    }

    /// Nu skips whitespace there, and what its `additional_whitespace` argument adds to it.
    /// It skips comments too, where `skip_comment` is set.
    #[expect(
        clippy::match_same_arms,
        reason = "one arm per recorded mode, to check each against nu on its own"
    )]
    fn skipped_in(mode: Mode) -> &'static [SyntaxKind] {
        match mode {
            Mode::Block => &[Whitespace],
            Mode::Paren => &[Whitespace, Newline, Comment],
            Mode::List => &[Whitespace, Newline, Comment, Comma],
            Mode::Record => &[Whitespace, Newline, Comment, Comma],
            Mode::RecordValue => &[Whitespace, Newline, Comment, Comma],
            Mode::Brace => &[Whitespace, Newline, Comment],
            // Nu adds nothing to whitespace here, so a line break would be a token.
            // It still sets `skip_comment`, though no row of `NU_VAR_TYPE_SPLITS` can show either.
            // Nu builds the type span out of one pipeline element, which ends at a line break.
            // A comment runs to the end of its line, so it never sits inside one element.
            Mode::VarType => &[Whitespace, Comment],
            // No row can show whitespace or a line break skipped, since nu lexes one word here.
            Mode::CellPath => &[Whitespace, Newline, Comment],
            // Nu adds nothing to whitespace here, so a line break is a token, as in `Block`.
            // Unlike there, it sets `skip_comment`.
            Mode::RangeFrom => &[Whitespace, Comment],
            Mode::Binary => &[Whitespace, Newline, Comment, Comma],
            Mode::RecordPattern => &[Whitespace, Newline, Comment, Comma],
            Mode::MatchArms => &[Whitespace, Newline, Comment, Comma],
            Mode::IoTypes => &[Whitespace, Newline, Comment, Comma],
            Mode::TypeParams => &[Whitespace, Newline, Comment],
            Mode::Signature => unimplemented!("how nu splits in {mode:?} is not recorded"),
        }
    }

    fn kind_of(src: &str) -> SyntaxKind {
        let tokens = lexed(src);
        assert_eq!(tokens.len(), 1, "{src:?} is not one token: {tokens:?}");
        tokens[0].0
    }

    fn assert_one_item(item: &str) {
        assert_eq!(split(&format!("{item} z")), [item, "z"], "{item:?}");
    }

    fn first_bracket_spans(src: &str, bracket: TextRange) -> Option<bool> {
        lex_all(src).tokens[0].first_bracket_spans(bracket)
    }

    #[test]
    fn first_bracket_is_first_closed_at_top_level() {
        for (src, bracket, expected) in [
            ("[a]", range(0, 3), true),
            ("[[a]]", range(0, 5), true),
            ("[a]:", range(0, 3), true),
            ("0x[ff]", range(2, 6), true),
            ("[a]b]", range(0, 5), false),
            ("[a][b]", range(0, 6), false),
        ] {
            assert_eq!(first_bracket_spans(src, bracket), Some(expected), "{src:?}");
        }
    }

    #[test]
    fn first_bracket_closing_far_from_end_does_not_span_to_end() {
        let src = format!("[a]{}]", "x".repeat(300));
        assert_eq!(
            first_bracket_spans(&src, TextRange::up_to(TextSize::of(src.as_str()))),
            Some(false)
        );
    }

    #[test]
    fn first_bracket_is_unknown_without_bracket_or_far_from_start() {
        let at = |len: u32| {
            (
                format!("{}[a]", "x".repeat(len as usize)),
                range(len, len + 3),
            )
        };
        for (src, bracket) in [("a".to_owned(), range(0, 1)), at(255), at(300)] {
            assert_eq!(first_bracket_spans(&src, bracket), None, "{src:?}");
        }
    }

    #[test]
    fn empty_input_makes_no_tokens() {
        assert!(lexed("").is_empty());
    }

    #[test]
    fn each_trivia_is_one_token() {
        assert_eq!(lexed(" \t"), [(Whitespace, " \t")]);
        assert_eq!(lexed("\n"), [(Newline, "\n")]);
        assert_eq!(lexed("#"), [(Comment, "#")]);
    }

    #[test]
    fn separators_split_items_around_them() {
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
    fn comment_starts_only_at_token_start() {
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
    fn non_ascii_chars_never_split_tokens() {
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
    fn item_brackets_are_outermost_outside_strings_and_comments() {
        for (src, expected) in [
            (r#"a:(b{c})"(x)"[d]r#'{'#"#, &["(b{c})", "[d]"][..]),
            ("a{b # }\n}", &["{b # }\n}"]),
        ] {
            let brackets: Vec<_> =
                item_brackets(src, TextRange::up_to(TextSize::of(src)), Mode::Block)
                    .into_iter()
                    .map(|range| &src[range])
                    .collect();
            assert_eq!(brackets, expected, "{src:?}");
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

    /// `à` (`C3 A0`) ends in a byte Latin-1 makes whitespace, so a comment starts inside the item.
    #[test]
    fn hash_after_latin1_whitespace_byte_starts_comment() {
        assert_eq!(split(r#"à#"b c"#), [r#"à#"b"#, "c"]);
        assert_eq!(split(r#"a#"b c""#), [r#"a#"b c""#]);
    }

    #[test]
    fn raw_string_closes_at_first_closer() {
        assert_eq!(kind_of("r##'a '# b'##"), RawString);
        assert_eq!(split("r#'a'# r#'b'#"), ["r#'a'#", "r#'b'#"]);
        assert_eq!(kind_of("r#'#"), RawString);
    }

    #[test]
    fn pipe_joins_redirection_but_not_append() {
        assert_eq!(split("e>|cat"), ["e>|", "cat"]);
        assert_eq!(split("o+e>|cat"), ["o+e>|", "cat"]);
        assert_eq!(split("err>|cat"), ["err>|", "cat"]);
        assert_eq!(split("e>>|cat"), ["e>>", "|", "cat"]);
        assert_eq!(split("o>>|cat"), ["o>>", "|", "cat"]);
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

    #[test]
    fn operators_get_their_kinds() {
        for &(expected, spellings) in OPERATORS {
            for &text in spellings {
                assert_eq!(kind_of(text), expected, "{text:?}");
            }
        }
    }

    #[test]
    fn spellings_nu_rejects_stay_words() {
        for text in ["o>|", "out>|", "&&", "2>", "2>&1"] {
            assert_eq!(kind_of(text), Word, "{text:?}");
        }
    }

    #[test]
    fn only_spellings_nu_rejects_take_kinds_nu_lexes_them_as() {
        for (text, expected) in [
            ("o>|", Some(Pipe)),
            ("out>|", Some(Pipe)),
            ("&&", Some(Pipe)),
            ("2>&1", Some(Pipe)),
            ("2>", Some(ErrGt)),
            ("||", None),
            ("e>|", None),
            ("2>>", None),
        ] {
            assert_eq!(rejected_operator_kind(text), expected, "{text:?}");
        }
    }

    #[test]
    fn unclosed_item_runs_to_end() {
        assert_eq!(
            lexed(r#"a "b c"#),
            [(Word, "a"), (Whitespace, " "), (Error, r#""b c"#)]
        );
        assert_eq!(kind_of("[a\n(b c"), Error);
        assert_eq!(kind_of(r#"$"(a" b"#), Error);
        assert_eq!(kind_of("r#'a' b"), Error);
    }

    #[test]
    fn error_ends_item_where_found() {
        assert_eq!(lexed("a}b"), [(Error, "a}"), (Word, "b")]);
        assert_eq!(lexed("a)b"), [(Error, "a)"), (Word, "b")]);
        assert_eq!(lexed("[a)b]"), [(Error, "[a)"), (Word, "b]")]);
        assert_eq!(lexed("{[}]"), [(Error, "{[}"), (Word, "]")]);
        assert_eq!(lexed("r#x"), [(Error, "r#"), (Word, "x")]);
    }

    #[test]
    fn final_backslash_splits_only_from_outermost_string() {
        assert_eq!(lexed(r#""a\"#), [(Error, r#""a"#), (Word, r"\")]);
        assert_eq!(lexed(r#"$"("\"#), [(Error, r#"$"("\"#)]);
    }

    #[test]
    fn unclosed_points_at_innermost_opener() {
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
    fn unclosed_interpolation_points_at_outermost_paren() {
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
    fn unmatched_closer_points_at_closer() {
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

    const NU_ERRORS: &[(&str, LexErrorKind)] = &[
        (r#"echo "a\"#, Unclosed),
        ("echo r#x", MissingRawQuote),
        ("echo a}b", UnmatchedCloser),
    ];

    #[test]
    fn block_splits_match_nu() {
        for &(src, expected) in NU_SPLITS {
            assert_eq!(split(src), expected, "{src:?}");
        }
    }

    const NU_PAREN_SPLITS: &[(&str, &[&str])] = &[
        ("a\nb", &["a", "b"]),
        ("a\r\nb", &["a", "b"]),
        ("a # c\nb", &["a", "b"]),
        ("a#b", &["a#b"]),
        ("a,b", &["a,b"]),
        ("a:b", &["a:b"]),
        ("a;b", &["a", ";", "b"]),
        ("a|b", &["a", "|", "b"]),
    ];

    #[test]
    fn paren_splits_match_nu() {
        for &(src, expected) in NU_PAREN_SPLITS {
            assert_eq!(split_in(src, Mode::Paren), expected, "{src:?}");
        }
    }

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
        ("a #,b\nc", &["a", "c"]),
        ("# c\na", &["a"]),
        ("a},b", &["a}", "b"]),
        ("[a,b", &["[a,b"]),
    ];

    #[test]
    fn list_splits_match_nu() {
        for &(src, expected) in NU_LIST_SPLITS {
            assert_eq!(split_in(src, Mode::List), expected, "{src:?}");
        }
    }

    const NU_RECORD_SPLITS: &[(&str, &[&str])] = &[
        ("a: 1", &["a", ":", "1"]),
        ("a:1,b:2", &["a", ":", "1", "b", ":", "2"]),
        (r#""a:b": 1"#, &[r#""a:b""#, ":", "1"]),
        (r#"a: "x:y""#, &["a", ":", r#""x:y""#]),
        (":a", &[":", "a"]),
        ("a#b: 1", &["a#b", ":", "1"]),
        ("à#b:c", &["à#b", ":", "c"]),
        ("a: [1, 2]", &["a", ":", "[1, 2]"]),
        ("a: {b: 1}", &["a", ":", "{b: 1}"]),
        ("a: 1\nb: 2", &["a", ":", "1", "b", ":", "2"]),
        ("a: 1 # c: d\nb: 2", &["a", ":", "1", "b", ":", "2"]),
        ("a=b.c?d!", &["a=b.c?d!"]),
    ];

    #[test]
    fn record_splits_match_nu() {
        for &(src, expected) in NU_RECORD_SPLITS {
            assert_eq!(split_in(src, Mode::Record), expected, "{src:?}");
        }
    }

    const NU_BRACE_SPLITS: &[(&str, &[&str])] = &[
        ("a: 1", &["a", ":", "1"]),
        ("a :b", &["a", ":", "b"]),
        ("a,b: 1", &["a,b", ":", "1"]),
        ("a\n: 1", &["a", ":", "1"]),
        ("a # b: c\n: 1", &["a", ":", "1"]),
        ("|x| a", &["|", "x", "|", "a"]),
        ("...$x, a: 1", &["...$x,", "a", ":", "1"]),
        ("a=b.c?d!", &["a=b.c?d!"]),
    ];

    #[test]
    fn brace_splits_match_nu() {
        for &(src, expected) in NU_BRACE_SPLITS {
            assert_eq!(split_in(src, Mode::Brace), expected, "{src:?}");
        }
    }

    const NU_RECORD_VALUE_SPLITS: &[(&str, &[&str])] = &[
        ("b:c", &["b:c"]),
        ("b:c,d", &["b:c", "d"]),
        ("x:#c", &["x:#c"]),
        ("b:c #x\nd", &["b:c", "d"]),
        ("b\r\nc", &["b", "c"]),
    ];

    #[test]
    fn record_value_splits_match_nu() {
        for &(src, expected) in NU_RECORD_VALUE_SPLITS {
            assert_eq!(split_in(src, Mode::RecordValue), expected, "{src:?}");
        }
    }

    const NU_VAR_TYPE_SPLITS: &[(&str, &[&str])] = &[
        ("record<a: int, b: string>", &["record<a: int, b: string>"]),
        ("int, string", &["int", ",", "string"]),
        ("a,<b,c>", &["a", ",", "<b,c>"]),
        ("a< b >c d", &["a< b >c", "d"]),
        ("a<(b)>", &["a<(b)>"]),
        ("a>b", &["a>b"]),
        ("a:b", &["a:b"]),
    ];

    #[test]
    fn var_type_splits_match_nu() {
        for &(src, expected) in NU_VAR_TYPE_SPLITS {
            assert_eq!(split_in(src, Mode::VarType), expected, "{src:?}");
        }
    }

    const NU_IO_TYPES_SPLITS: &[(&str, &[&str])] = &[
        (
            "int -> string, nothing -> int",
            &["int", "->", "string", "nothing", "->", "int"],
        ),
        (
            "record<a: int, b: string> -> list<int>",
            &["record<a: int, b: string>", "->", "list<int>"],
        ),
        ("int\r\n-> int", &["int", "->", "int"]),
        ("int # c\n-> int", &["int", "->", "int"]),
        ("a<b,c>", &["a<b,c>"]),
        ("a<\nb>", &["a<\nb>"]),
        ("record<a: int # c\n>", &["record<a: int # c\n>"]),
        ("list<int -> int", &["list<int ->", "int"]),
        ("int#c", &["int#c"]),
        ("a:b=c", &["a:b=c"]),
        ("a|b", &["a", "|", "b"]),
        ("\"a b\" -> c", &["\"a b\"", "->", "c"]),
    ];

    #[test]
    fn io_types_splits_match_nu() {
        for &(src, expected) in NU_IO_TYPES_SPLITS {
            assert_eq!(split_in(src, Mode::IoTypes), expected, "{src:?}");
        }
    }

    const NU_TYPE_PARAMS_SPLITS: &[(&str, &[&str])] = &[
        (
            "a: int, b: string",
            &["a", ":", "int", ",", "b", ":", "string"],
        ),
        ("a:\r\nint", &["a", ":", "int"]),
        ("a # c\n: int", &["a", ":", "int"]),
        ("a: list<b: c>", &["a", ":", "list<b: c>"]),
        ("\"a:b\": int", &["\"a:b\"", ":", "int"]),
        ("a,,b", &["a", ",", ",", "b"]),
        ("a=b", &["a=b"]),
        ("a|b", &["a", "|", "b"]),
    ];

    #[test]
    fn type_params_splits_match_nu() {
        for &(src, expected) in NU_TYPE_PARAMS_SPLITS {
            assert_eq!(split_in(src, Mode::TypeParams), expected, "{src:?}");
        }
    }

    const NU_MATCH_ARMS_SPLITS: &[(&str, &[&str])] = &[
        ("1 => a, 2 => b", &["1", "=>", "a", "2", "=>", "b"]),
        ("1\n=> a", &["1", "=>", "a"]),
        ("a\r\nb", &["a", "b"]),
        ("1 # c\n=> a", &["1", "=>", "a"]),
        ("1|2 => a", &["1", "|", "2", "=>", "a"]),
        ("1=>a", &["1=>a"]),
        ("[a, b] => c", &["[a, b]", "=>", "c"]),
        ("a;b", &["a", ";", "b"]),
        ("{|x| $x} => c", &["{|x| $x}", "=>", "c"]),
        ("a ||b", &["a", "||", "b"]),
    ];

    #[test]
    fn match_arms_splits_match_nu() {
        for &(src, expected) in NU_MATCH_ARMS_SPLITS {
            assert_eq!(split_in(src, Mode::MatchArms), expected, "{src:?}");
        }
    }

    const NU_CELL_PATH_SPLITS: &[(&str, &[&str])] = &[
        ("$x.a", &["$x", ".", "a"]),
        (
            "$x.a?!.b!?",
            &["$x", ".", "a", "?", "!", ".", "b", "!", "?"],
        ),
        ("$x.?", &["$x", ".", "?"]),
        ("$x.a.", &["$x", ".", "a", "."]),
        (r#"$x."a.b".c"#, &["$x", ".", r#""a.b""#, ".", "c"]),
        ("$x.'a?'.b", &["$x", ".", "'a?'", ".", "b"]),
        ("$x.`a!`", &["$x", ".", "`a!`"]),
        ("$x.r#'a.b'#", &["$x", ".", "r#'a.b'#"]),
        ("$x.a[0].b", &["$x", ".", "a[0]", ".", "b"]),
        ("$x.a{b.c}", &["$x", ".", "a{b.c}"]),
        ("$x.a(b.c)", &["$x", ".", "a(b.c)"]),
        ("$x.a<b.c>", &["$x", ".", "a<b", ".", "c>"]),
        ("$x.a-b", &["$x", ".", "a-b"]),
        ("$x.a:b", &["$x", ".", "a:b"]),
        ("$x.a,b", &["$x", ".", "a,b"]),
        ("$x.a=b", &["$x", ".", "a=b"]),
        ("$x.a#b.c", &["$x", ".", "a#b", ".", "c"]),
        ("$x.#a", &["$x", "."]),
        ("$x.a?#b", &["$x", ".", "a", "?"]),
        (r#"("a.b").c"#, &[r#"("a.b")"#, ".", "c"]),
        (r#"["a.b"].c"#, &[r#"["a.b"]"#, ".", "c"]),
        (r#"{a: "b.c"}.d"#, &[r#"{a: "b.c"}"#, ".", "d"]),
    ];

    #[test]
    fn cell_path_splits_match_nu() {
        for &(src, expected) in NU_CELL_PATH_SPLITS {
            assert_eq!(split_in(src, Mode::CellPath), expected, "{src:?}");
        }
    }

    const NU_RANGE_FROM_SPLITS: &[(&str, &[&str])] = &[
        ("1", &["1"]),
        ("-1.5", &["-1", ".", "5"]),
        ("$x.a?!", &["$x", ".", "a", "?", "!"]),
        ("$x..", &["$x", ".", "."]),
        (r#"("a.b").c"#, &[r#"("a.b")"#, ".", "c"]),
        ("$x.a#b", &["$x", ".", "a#b"]),
        ("$x.a\nb", &["$x", ".", "a", "\n", "b"]),
        ("$x.#(\n)", &["$x", ".", "\n", ")"]),
        (r#"$x."a"#, &["$x", ".", r#""a"#]),
        ("$x.[a", &["$x", ".", "[a"]),
        ("a)b", &["a)", "b"]),
    ];

    /// Nu reads no range where it reports a lexing error.
    const NU_RANGE_FROM_ERRORS: &[(&str, LexErrorKind)] = &[
        ("$x.#(\n)", UnmatchedCloser),
        (r#"$x."a"#, Unclosed),
        ("$x.[a", Unclosed),
        ("a)b", UnmatchedCloser),
    ];

    #[test]
    fn range_from_splits_match_nu() {
        for &(src, expected) in NU_RANGE_FROM_SPLITS {
            assert_eq!(split_in(src, Mode::RangeFrom), expected, "{src:?}");
        }
    }

    #[test]
    fn range_from_errors_match_nu() {
        for &(src, _) in NU_RANGE_FROM_SPLITS {
            let kinds: Vec<_> = lex_all_in(src, Mode::RangeFrom)
                .errors
                .iter()
                .map(|error| error.kind)
                .collect();
            let expected = NU_RANGE_FROM_ERRORS
                .iter()
                .find(|&&(row, _)| row == src)
                .map(|&(_, kind)| kind);
            assert_eq!(kinds, expected.as_slice(), "{src:?}");
        }
    }

    const NU_BINARY_SPLITS: &[(&str, &[&str])] = &[
        ("ff 00", &["ff", "00"]),
        ("ff,00", &["ff", "00"]),
        ("ff\n00", &["ff", "00"]),
        ("ff # c\n00", &["ff", "00"]),
        ("ff,#c\n00", &["ff", "00"]),
        ("ff;00", &["ff", ";", "00"]),
        ("ff|00", &["ff", "|", "00"]),
        ("ff = 00", &["ff", "=", "00"]),
        (r#"f"f""#, &[r#"f"f""#]),
        ("a:b", &["a:b"]),
        ("a=b", &["a=b"]),
    ];

    #[test]
    fn binary_splits_match_nu() {
        for &(src, expected) in NU_BINARY_SPLITS {
            assert_eq!(split_in(src, Mode::Binary), expected, "{src:?}");
        }
    }

    const NU_RECORD_PATTERN_SPLITS: &[(&str, &[&str])] = &[
        ("a: 1, $b", &["a", ":", "1", "$b"]),
        ("a:b:c", &["a", ":", "b", ":", "c"]),
        (r#""a b": 1"#, &[r#""a b""#, ":", "1"]),
        ("a # c\n: 1", &["a", ":", "1"]),
        ("a,#c\n:1", &["a", ":", "1"]),
        ("a:\r\nb", &["a", ":", "b"]),
        ("a:{b: 1}", &["a", ":", "{b: 1}"]),
        ("a: [1, 2]", &["a", ":", "[1, 2]"]),
        ("a:b=c", &["a", ":", "b=c"]),
        ("a;b", &["a", ";", "b"]),
        ("a|b", &["a", "|", "b"]),
        ("a = b", &["a", "=", "b"]),
        ("a:#b", &["a", ":"]),
        ("a::b", &["a", ":", ":", "b"]),
        ("$a:1", &["$a", ":", "1"]),
    ];

    #[test]
    fn record_pattern_splits_match_nu() {
        for &(src, expected) in NU_RECORD_PATTERN_SPLITS {
            assert_eq!(split_in(src, Mode::RecordPattern), expected, "{src:?}");
        }
    }

    #[test]
    fn angle_brackets_nest_only_in_var_type() {
        for (src, error) in [
            ("a<b", (Unclosed, "<")),
            ("a<b<c>", (Unclosed, "<")),
            ("a<[b>]", (Unclosed, "<")),
            ("a<b # c>", (Unclosed, "<")),
            ("a<b]", (UnmatchedCloser, "]")),
            ("a<b}", (UnmatchedCloser, "}")),
            ("a<b)", (UnmatchedCloser, ")")),
        ] {
            let Lexed {
                tokens,
                errors,
            } = lex_all_in(src, Mode::VarType);
            assert_eq!(tokens.len(), 1, "{src:?}");
            assert_eq!(errors.len(), 1, "{src:?}");
            assert_eq!((errors[0].kind, &src[errors[0].range]), error, "{src:?}");
        }
        for mode in [
            Mode::Block,
            Mode::Paren,
            Mode::List,
            Mode::Record,
            Mode::RecordValue,
            Mode::Brace,
            Mode::MatchArms,
            Mode::CellPath,
            Mode::RangeFrom,
            Mode::Binary,
            Mode::RecordPattern,
        ] {
            assert_eq!(lexed_in("a<b", mode), [(Word, "a<b")], "{mode:?}");
        }
    }

    #[test]
    fn colon_is_token_in_record_and_brace_modes() {
        for mode in [Mode::Record, Mode::Brace] {
            assert_eq!(
                lexed_in("a:b", mode),
                [(Word, "a"), (Colon, ":"), (Word, "b")],
                "{mode:?}"
            );
        }
    }

    #[test]
    fn special_bytes_are_tokens_at_item_start() {
        for &mode in Mode::ALL {
            for &(byte, kind) in mode.special() {
                let special = char::from(byte).to_string();
                let src = format!("{special}a{special}{special}b {special}");
                assert_eq!(
                    lexed_in(&src, mode),
                    [
                        (kind, special.as_str()),
                        (Word, "a"),
                        (kind, special.as_str()),
                        (kind, special.as_str()),
                        (Word, "b"),
                        (Whitespace, " "),
                        (kind, special.as_str()),
                    ],
                    "{mode:?} {src:?}"
                );
            }
        }
    }

    #[test]
    fn items_before_special_byte_keep_kinds() {
        for (src, kind) in [
            (r#""a",b"#, SyntaxKind::String),
            (r#"$"(a,b)",b"#, InterpString),
            ("r#'a,b'#,b", RawString),
            ("e>,b", ErrGt),
            ("e>|,b", ErrGtPipe),
            ("=,b", Assign),
        ] {
            let tokens = lexed_in(src, Mode::List);
            assert_eq!(tokens[0].0, kind, "{src:?}");
            assert_eq!(&tokens[1..], [(Comma, ","), (Word, "b")], "{src:?}");
        }
    }

    #[test]
    fn block_errors_match_nu() {
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
            lexed_ranges("# ab\n", range(0, 3)),
            [(Comment, range(0, 3))]
        );
        assert_eq!(
            lexed_ranges(r#""a b""#, range(0, 3)),
            [(Error, range(0, 3))]
        );
        assert!(lexed_ranges("abcd", range(2, 2)).is_empty());
    }

    #[test]
    fn bytes_before_range_are_ignored() {
        assert_eq!(
            lexed_ranges(r#"$"(" x"#, range(1, 6)),
            [
                (SyntaxKind::String, range(1, 4)),
                (Whitespace, range(4, 5)),
                (Word, range(5, 6)),
            ]
        );
        assert_eq!(
            lex(r#"$"a"#, range(1, 3), Mode::Block).errors,
            [LexError {
                kind: Unclosed,
                range: range(1, 2),
            }]
        );
    }

    #[test]
    #[should_panic(expected = "range splits a char")]
    fn range_off_char_boundary_panics() {
        lex("日", range(0, 1), Mode::Block);
    }

    #[test]
    #[should_panic(expected = "range is outside src")]
    fn range_outside_src_panics() {
        lex("ab", range(1, 3), Mode::Block);
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
        fn unclosed_opener_includes_prefix() {
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
        fn missing_raw_quote_points_at_prefix() {
            assert_eq!(error_of("r#x"), (MissingRawQuote, "r#"));
            assert_eq!(error_of("xr##"), (MissingRawQuote, "r##"));
        }
    }
}
