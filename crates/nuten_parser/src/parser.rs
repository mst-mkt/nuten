//! Parser that turns tokens into events for the tree builder.

use std::{mem, vec};

use nuten_syntax::SyntaxKind;
use rowan::{TextRange, TextSize};

use crate::{
    ParseError, ParseErrorKind, grammar,
    lexer::{self, LexError, Lexed, Mode, Token},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Event {
    Start(SyntaxKind),
    Token(Token),
    Error(ParseError),
    Finish,
}

/// How deep brackets may nest before a bracketed word stays a word.
const MAX_DEPTH: usize = 128;

/// A pair of one-byte brackets and the kinds of their tokens.
pub(crate) struct Brackets {
    pub(crate) open: u8,
    pub(crate) close: u8,
    pub(crate) open_kind: SyntaxKind,
    pub(crate) close_kind: SyntaxKind,
}

/// What `Parser::bracketed` found in the next token.
pub(crate) struct Bracketed<'a> {
    inner: Parser<'a>,
    token: Token,
    brackets: &'static Brackets,
}

impl<'a> Bracketed<'a> {
    /// A parser of what the token holds, to look into before emitting it.
    pub(crate) fn inner(&self) -> &Parser<'a> {
        &self.inner
    }
}

pub(crate) fn parse(src: &str, lexed: Lexed) -> Vec<Event> {
    let mut parser = Parser::new(src, lexed, 0);
    // Not `start`: that would put leading trivia outside Root.
    parser.events.push(Event::Start(SyntaxKind::Root));
    grammar::file(&mut parser);
    parser.bump_skipped();
    parser.events.push(Event::Finish);
    parser.events
}

/// Whitespace and comments are skipped when looking ahead and emitted
/// just before the next token or node, or at the end of the tokens,
/// so no node but Root starts or ends with them.
/// Newlines are not skipped, since they end statements.
pub(crate) struct Parser<'a> {
    src: &'a str,
    tokens: Vec<Token>,
    lex_errors: vec::IntoIter<LexError>,
    pos: usize,
    events: Vec<Event>,
    /// Brackets around the tokens.
    depth: usize,
}

impl<'a> Parser<'a> {
    fn new(src: &'a str, lexed: Lexed, depth: usize) -> Parser<'a> {
        Parser {
            src,
            tokens: lexed.tokens,
            lex_errors: lexed.errors.into_iter(),
            pos: 0,
            events: Vec::new(),
            depth,
        }
    }

    /// The kind of the next token, or `None` at the end.
    pub(crate) fn peek(&self) -> Option<SyntaxKind> {
        self.nth(0)
    }

    /// The text of the next token, or `None` at the end.
    pub(crate) fn peek_text(&self) -> Option<&'a str> {
        self.nth_text(0)
    }

    /// The kind of the `n`th token ahead, `0` being the next, or `None` past
    /// the end.
    pub(crate) fn nth(&self, n: usize) -> Option<SyntaxKind> {
        self.nth_token(n).map(|token| token.kind)
    }

    /// The text of the `n`th token ahead, `0` being the next, or `None` past
    /// the end.
    pub(crate) fn nth_text(&self, n: usize) -> Option<&'a str> {
        self.nth_token(n).map(|token| &self.src[token.range])
    }

    /// The kind of the last token emitted, trivia aside.
    pub(crate) fn last_kind(&self) -> Option<SyntaxKind> {
        self.tokens[..self.pos]
            .iter()
            .rev()
            .map(|token| token.kind)
            .find(|kind| !kind.is_trivia())
    }

    /// The kinds of every token ahead, whitespace and comments included.
    /// Only for looking past what `peek` sees.
    pub(crate) fn remaining_kinds(&self) -> impl Iterator<Item = SyntaxKind> {
        self.tokens[self.pos..].iter().map(|token| token.kind)
    }

    /// As `remaining_kinds`, with each token and its text.
    pub(crate) fn remaining_tokens(&self) -> impl Iterator<Item = (Token, &'a str)> {
        self.tokens[self.pos..]
            .iter()
            .map(|&token| (token, &self.src[token.range]))
    }

    /// The first token of lexing again in `mode`, from `start` to where the
    /// tokens end.
    ///
    /// # Panics
    ///
    /// Panics when `start` is past where the tokens end.
    pub(crate) fn lex_first(&self, start: TextSize, mode: Mode) -> Option<Token> {
        let end = self.tokens.last()?.range.end();
        lexer::lex_first(self.src, TextRange::new(start, end), mode)
    }

    /// Emits the next `count` tokens, which touch one another, as one `Word`.
    ///
    /// # Panics
    ///
    /// Panics when `count` is 0 or more than are left, when they do not touch,
    /// or when one is trivia or an `Error`.
    pub(crate) fn bump_joined(&mut self, count: usize) {
        self.bump_skipped();
        let tokens = &self.tokens[self.pos..self.pos + count];
        assert!(
            !tokens.is_empty()
                && tokens
                    .windows(2)
                    .all(|pair| pair[0].range.end() == pair[1].range.start())
                && tokens
                    .iter()
                    .all(|token| !token.kind.is_trivia() && token.kind != SyntaxKind::Error),
            "bump_joined of tokens that do not make one word"
        );
        let range = TextRange::new(tokens[0].range.start(), tokens[count - 1].range.end());
        self.pos += count;
        self.push_token(SyntaxKind::Word, range);
    }

    /// What the next token holds inside `brackets`, lexed in `mode`.
    /// `None` when the token is not a word enclosed in them, or when brackets
    /// nest too deep.
    pub(crate) fn bracketed(
        &self,
        brackets: &'static Brackets,
        mode: Mode,
    ) -> Option<Bracketed<'a>> {
        let token = self.nth_token(0)?;
        if token.kind != SyntaxKind::Word
            || self.depth == MAX_DEPTH
            || !encloses(self.src, token.range, brackets)
        {
            return None;
        }
        let (_, inside, _) = split(token.range);
        let lexed = lexer::lex(self.src, inside, mode);
        Some(Bracketed {
            inner: Parser::new(self.src, lexed, self.depth + 1),
            token,
            brackets,
        })
    }

    /// Emits the next token as a `node`: its opening bracket, the events
    /// `items` makes of what it holds, and its closing bracket.
    ///
    /// # Panics
    ///
    /// Panics unless `bracketed` was found in the next token, or when `items`
    /// leaves any token of what it holds.
    pub(crate) fn bump_bracketed(
        &mut self,
        node: SyntaxKind,
        bracketed: Bracketed<'a>,
        items: fn(&mut Parser<'a>),
    ) {
        let Bracketed {
            mut inner,
            token,
            brackets,
        } = bracketed;
        assert_eq!(
            self.nth_token(0),
            Some(token),
            "bump_bracketed at another token"
        );
        self.start(node);
        self.pos += 1;
        let (open, _, close) = split(token.range);
        self.push_token(brackets.open_kind, open);
        // Lent, so that nested brackets do not copy the events inside.
        inner.events = mem::take(&mut self.events);
        items(&mut inner);
        inner.bump_skipped();
        assert!(
            inner.pos == inner.tokens.len(),
            "items left some of what the token holds"
        );
        self.events = inner.events;
        self.push_token(brackets.close_kind, close);
        self.finish();
    }

    /// Emits the next token.
    ///
    /// # Panics
    ///
    /// Panics at the end.
    pub(crate) fn bump(&mut self) {
        self.bump_skipped();
        assert!(self.pos < self.tokens.len(), "bump at the end");
        self.bump_one();
    }

    /// Emits the next token inside an `Error` node, reporting `kind` at it.
    ///
    /// # Panics
    ///
    /// Panics at the end, or at an `Error` token, which already carries an
    /// error.
    pub(crate) fn bump_error(&mut self, kind: ParseErrorKind) {
        let token = self.nth_token(0).expect("bump_error at the end");
        assert!(
            token.kind != SyntaxKind::Error,
            "bump_error at an Error token"
        );
        self.start(SyntaxKind::Error);
        self.bump();
        self.events.push(Event::Error(ParseError {
            kind,
            range: token.range,
        }));
        self.finish();
    }

    pub(crate) fn start(&mut self, kind: SyntaxKind) {
        self.bump_skipped();
        self.events.push(Event::Start(kind));
    }

    pub(crate) fn finish(&mut self) {
        self.events.push(Event::Finish);
    }

    fn nth_token(&self, n: usize) -> Option<Token> {
        self.tokens[self.pos..]
            .iter()
            .copied()
            .filter(|token| !is_skipped(token.kind))
            .nth(n)
    }

    fn bump_skipped(&mut self) {
        while self
            .tokens
            .get(self.pos)
            .is_some_and(|token| is_skipped(token.kind))
        {
            self.bump_one();
        }
    }

    fn bump_one(&mut self) {
        let token = self.tokens[self.pos];
        self.pos += 1;
        self.events.push(Event::Token(token));
        if token.kind == SyntaxKind::Error {
            let error = self.lex_errors.next().expect("an error per Error token");
            self.events.push(Event::Error(error.into()));
        }
    }

    fn push_token(&mut self, kind: SyntaxKind, range: TextRange) {
        self.events.push(Event::Token(Token {
            kind,
            range,
        }));
    }
}

fn is_skipped(kind: SyntaxKind) -> bool {
    matches!(kind, SyntaxKind::Whitespace | SyntaxKind::Comment)
}

/// Whether the word at `range` of `src` starts with `brackets` that close at
/// its last byte, not before as in `[a].b]`: without that byte, the only
/// error is its first bracket left unclosed.
fn encloses(src: &str, range: TextRange, brackets: &Brackets) -> bool {
    let bytes = src[range].as_bytes();
    if bytes.first() != Some(&brackets.open) || bytes.last() != Some(&brackets.close) {
        return false;
    }
    let (open, _, close) = split(range);
    let lexed = lexer::lex(
        src,
        TextRange::new(range.start(), close.start()),
        Mode::block(),
    );
    matches!(lexed.errors.as_slice(), [error] if error.range == open)
}

/// The first byte of `range`, what lies between, and the last byte.
fn split(range: TextRange) -> (TextRange, TextRange, TextRange) {
    let one = TextSize::new(1);
    let (start, end) = (range.start() + one, range.end() - one);
    (
        TextRange::new(range.start(), start),
        TextRange::new(start, end),
        TextRange::new(end, range.end()),
    )
}

#[cfg(test)]
mod tests {
    use nuten_syntax::SyntaxKind;
    use rowan::{TextRange, TextSize};

    use super::Parser;
    use crate::{
        grammar::SQUARE,
        lexer::{self, Mode},
    };

    fn parser(src: &str) -> Parser<'_> {
        let lexed = lexer::lex(src, TextRange::up_to(TextSize::of(src)), Mode::block());
        Parser::new(src, lexed, 0)
    }

    #[test]
    #[should_panic(expected = "bump_bracketed at another token")]
    fn bump_bracketed_emits_only_the_token_bracketed_looked_at() {
        let mut p = parser("[a] [b]");
        let first = p.bracketed(&SQUARE, Mode::list()).unwrap();
        p.bump();
        p.bump_bracketed(SyntaxKind::List, first, |_| {});
    }

    #[test]
    #[should_panic(expected = "items left some of what the token holds")]
    fn bump_bracketed_needs_items_to_take_everything() {
        let mut p = parser("[a]");
        let bracketed = p.bracketed(&SQUARE, Mode::list()).unwrap();
        p.bump_bracketed(SyntaxKind::List, bracketed, |_| {});
    }
}
