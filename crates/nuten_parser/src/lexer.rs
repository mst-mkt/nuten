//! Lexer ported from `lex.rs` in nu-parser.

use std::{fmt, ops::Range};

use nuten_syntax::SyntaxKind;
use rowan::{TextRange, TextSize};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Token {
    pub(crate) kind: SyntaxKind,
    pub(crate) range: TextRange,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Lexed {
    pub(crate) tokens: Vec<Token>,
    /// One error per `Error` token, in the same order.
    pub(crate) errors: Vec<LexError>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct LexError {
    pub(crate) kind: LexErrorKind,
    /// Always inside its `Error` token.
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
    /// A quote or bracket that never closes.
    /// The range covers the innermost opener, such as `[`, `$"`, or `r#'`.
    /// In interpolation it covers the outermost unclosed `(` instead.
    Unclosed,
    /// A `]`, `}`, or `)` that does not match the innermost opener.
    /// A `]` with nothing open is the exception.
    UnmatchedCloser,
    /// `r#` not followed by `'`.
    /// The range covers `r` and the `#`s.
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

/// The part of nu's `lex` settings that decides where items end.
/// Each mode is one of the ways nu calls `lex`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Mode {
    /// Statements in the top level, `{ }` blocks, closures, and modules.
    /// Also the right-hand side of `let` and assignments.
    Block,
    /// The inside of the `( )` of a subexpression.
    /// Nu lexes a line break as whitespace there, so no statement ends at one.
    Paren,
    /// Lists, tables, and list patterns.
    List,
    /// The keys, spreads, and `:` of a record, in the inside of `{ }`.
    Record,
    /// A value of a record, where nu does not treat `:` as special.
    RecordValue,
    /// The inside of `{ }` looked into to tell a record from a closure or a block.
    Brace,
    /// The type of a variable, as in `let x: int`.
    VarType,
    /// The inside of the `[ ]` of a `def` or an `extern`.
    Signature,
    /// The input and output types after the signature, as in `]: nothing -> string`.
    IoTypes,
    /// The inside of the `< >` of a type, as in `record<a: int>`.
    TypeParams,
    /// The inside of the `{ }` of a `match`.
    MatchArms,
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
    ];

    /// Bytes that end an item, on top of whitespace, `|`, and `;`.
    /// Each comes with the kind of the token it makes on its own at the start of an item.
    /// Nu's special tokens, plus `,` where nu treats it as whitespace.
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
        }
    }

    /// Whether `<` and `>` nest as brackets, as nu's `lex_signature` does where it reads a type.
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

    pub(crate) fn special_kind(self, byte: u8) -> Option<SyntaxKind> {
        self.special()
            .iter()
            .find_map(|&(special, kind)| (special == byte).then_some(kind))
    }
}

/// Lexes `range` of `src` without looking at the bytes around it.
/// All ranges are absolute in `src`, and the tokens cover `range` exactly.
pub(crate) fn lex(src: &str, range: TextRange, mode: Mode) -> Lexed {
    let mut lexer = Lexer::new(src, range, mode);
    Lexed {
        tokens: lexer.by_ref().collect(),
        errors: lexer.errors,
    }
}

/// The first token of lexing `range` of `src`, without lexing the rest.
/// An `Error` token comes without its error.
pub(crate) fn lex_first(src: &str, range: TextRange, mode: Mode) -> Option<Token> {
    Lexer::new(src, range, mode).next()
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
    fn new(src: &'a str, range: TextRange, mode: Mode) -> Lexer<'a> {
        assert!(range.end() <= TextSize::of(src), "range is outside src");
        assert!(
            src.is_char_boundary(range.start().into()) && src.is_char_boundary(range.end().into()),
            "range splits a char"
        );
        Lexer {
            src: &src.as_bytes()[..usize::from(range.end())],
            pos: usize::from(range.start()),
            mode,
            errors: Vec::new(),
        }
    }

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
                // Nu ends the item before a final `\` in the outermost string.
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

/// Whether a `#` inside an item starts a comment, given the byte before it.
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
mod tests;
