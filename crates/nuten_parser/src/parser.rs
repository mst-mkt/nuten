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
    /// Whether nu's lexer nests them in every mode.
    /// Then lexing a word in block mode tells whether its first bracket closes at its last byte.
    /// It must not close before that, as in `[a].b]`.
    /// `< >` nest only where a type is read, so the caller checks what they hold by lexing it.
    pub(crate) always_nested: bool,
}

impl Brackets {
    /// Whether `range` of `src` starts with the opening bracket and ends with its closer.
    fn enclose(&self, src: &str, range: TextRange) -> bool {
        let bytes = src[range].as_bytes();
        if bytes.first() != Some(&self.open) || bytes.last() != Some(&self.close) {
            return false;
        }
        if !self.always_nested {
            return true;
        }
        // Without the last byte, the only error is the first bracket left unclosed.
        let (open, _, close) = split(range);
        let lexed = lexer::lex(
            src,
            TextRange::new(range.start(), close.start()),
            Mode::Block,
        );
        matches!(lexed.errors.as_slice(), [error] if error.range == open)
    }
}

/// The bytes of a bracketed word outside its brackets.
#[derive(Clone, Copy)]
pub(crate) enum Affix {
    None,
    /// This many bytes before the opening bracket, as the name of a type before its `< >`.
    /// `bump_bracketed` emits them as a `Word` inside the node.
    Name(usize),
    /// One byte after the closing bracket, such as the `:` before input and output types.
    /// `bump_bracketed` emits it as this kind after the node.
    Suffix(SyntaxKind),
}

impl Affix {
    fn brackets_within(self, range: TextRange) -> TextRange {
        match self {
            Affix::None => range,
            Affix::Name(len) => TextRange::new(
                range.start() + TextSize::new(u32::try_from(len).expect("a name of a word")),
                range.end(),
            ),
            Affix::Suffix(_) => TextRange::new(range.start(), range.end() - TextSize::new(1)),
        }
    }
}

/// What `Parser::bracketed` found in the next token.
pub(crate) struct Bracketed<'a> {
    inner: Parser<'a>,
    token: Token,
    /// The brackets and what they hold.
    range: TextRange,
    brackets: &'static Brackets,
    affix: Affix,
}

impl<'a> Bracketed<'a> {
    /// A parser of what the token holds, to look into before emitting it.
    pub(crate) fn inner(&self) -> &Parser<'a> {
        &self.inner
    }

    /// The same token, with what it holds lexed again in `mode`.
    pub(crate) fn lexed_in(self, mode: Mode) -> Bracketed<'a> {
        let Parser {
            src,
            depth,
            ..
        } = self.inner;
        let (_, inside, _) = split(self.range);
        let lexed = lexer::lex(src, inside, mode);
        Bracketed {
            inner: Parser::new(src, lexed, depth),
            ..self
        }
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

/// Whitespace and comments are skipped when looking ahead.
/// They are emitted just before the next token or node, or at the end of the tokens.
/// So no node but Root starts or ends with them.
/// Newlines are not skipped, since they end statements.
pub(crate) struct Parser<'a> {
    src: &'a str,
    tokens: Vec<Token>,
    lex_errors: vec::IntoIter<LexError>,
    pos: usize,
    events: Vec<Event>,
    /// Brackets around the tokens.
    depth: usize,
    /// How many errors were reported among these tokens.
    /// Those inside a bracketed word read as a node are not counted.
    /// Whether `{ }` reads as a block depends on its own statements alone.
    errors: usize,
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
            errors: 0,
        }
    }

    pub(crate) fn peek(&self) -> Option<SyntaxKind> {
        self.nth(0)
    }

    pub(crate) fn peek_text(&self) -> Option<&'a str> {
        self.nth_text(0)
    }

    /// The kind of the `n`th token ahead, `0` being the next, or `None` past the end.
    pub(crate) fn nth(&self, n: usize) -> Option<SyntaxKind> {
        self.nth_token(n).map(|token| token.kind)
    }

    /// The text of the `n`th token ahead, `0` being the next, or `None` past the end.
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

    pub(crate) fn lex_first(&self, start: TextSize, mode: Mode) -> Option<Token> {
        let end = self.tokens.last()?.range.end();
        lexer::lex_first(self.src, TextRange::new(start, end), mode)
    }

    pub(crate) fn lex_error(&self, range: TextRange, mode: Mode) -> Option<LexError> {
        lexer::lex(self.src, range, mode).errors.into_iter().next()
    }

    /// Emits the next `count` tokens, which touch one another, as one `Word`.
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
    /// `None` when the token is not a word enclosed in them, or when brackets nest too deep.
    pub(crate) fn bracketed(
        &self,
        brackets: &'static Brackets,
        mode: Mode,
    ) -> Option<Bracketed<'a>> {
        self.bracketed_affixed(brackets, mode, Affix::None)
    }

    /// As `bracketed`, for a word with `affix` outside its brackets.
    pub(crate) fn bracketed_affixed(
        &self,
        brackets: &'static Brackets,
        mode: Mode,
        affix: Affix,
    ) -> Option<Bracketed<'a>> {
        let token = self.nth_token(0)?;
        if token.kind != SyntaxKind::Word || self.depth == MAX_DEPTH {
            return None;
        }
        let range = affix.brackets_within(token.range);
        if !brackets.enclose(self.src, range) {
            return None;
        }
        let (_, inside, _) = split(range);
        let lexed = lexer::lex(self.src, inside, mode);
        Some(Bracketed {
            inner: Parser::new(self.src, lexed, self.depth + 1),
            token,
            range,
            brackets,
            affix,
        })
    }

    /// Emits the next token as a `node`.
    /// Its opening bracket, the events `items` makes of what it holds, then its closing bracket.
    /// The token stays a `Word` when a comment inside has no whitespace before its `#`.
    /// Nu reads a word with that `#` as no comment, and counts the brackets in its text.
    /// So whitespace put before the `#` may move where the word ends.
    pub(crate) fn bump_bracketed(
        &mut self,
        node: SyntaxKind,
        bracketed: Bracketed<'a>,
        items: impl FnOnce(&mut Parser<'a>),
    ) {
        self.emit_bracketed(node, bracketed, items);
    }

    /// As `bump_bracketed`, unless `items` reports an error among the tokens it holds.
    /// Then its events are dropped and the token stays a `Word`.
    /// Errors inside nested nodes do not count.
    /// For a word that nu may read as a string, where nothing inside may be reported.
    pub(crate) fn bump_bracketed_or_word(
        &mut self,
        node: SyntaxKind,
        bracketed: Bracketed<'a>,
        items: impl FnOnce(&mut Parser<'a>),
    ) {
        let token = bracketed.token;
        // Before the mark, so that the trivia before the word stays when the node is dropped.
        self.bump_skipped();
        let mark = self.events.len();
        if self.emit_bracketed(node, bracketed, items) > 0 {
            self.events.truncate(mark);
            self.push_token(SyntaxKind::Word, token.range);
        }
    }

    /// Returns how many errors `items` reported among the tokens it holds.
    fn emit_bracketed(
        &mut self,
        node: SyntaxKind,
        bracketed: Bracketed<'a>,
        items: impl FnOnce(&mut Parser<'a>),
    ) -> usize {
        let Bracketed {
            mut inner,
            token,
            range,
            brackets,
            affix,
        } = bracketed;
        assert_eq!(
            self.nth_token(0),
            Some(token),
            "bump_bracketed at another token"
        );
        if has_glued_comment(self.src, &inner.tokens) {
            self.bump();
            return 0;
        }
        let (open, _, close) = split(range);
        self.start(node);
        self.pos += 1;
        if let Affix::Name(_) = affix {
            self.push_token(
                SyntaxKind::Word,
                TextRange::new(token.range.start(), open.start()),
            );
        }
        self.push_token(brackets.open_kind, open);
        // Lent, so that nested brackets do not copy the events inside.
        inner.events = mem::take(&mut self.events);
        items(&mut inner);
        inner.bump_skipped();
        assert!(
            inner.pos == inner.tokens.len(),
            "items left some of what the token holds"
        );
        self.events = mem::take(&mut inner.events);
        self.push_token(brackets.close_kind, close);
        self.finish();
        if let Affix::Suffix(kind) = affix {
            self.push_token(kind, TextRange::new(close.end(), token.range.end()));
        }
        inner.errors
    }

    /// Emits the whitespace and comments ahead, so that a node does not start or end with them.
    pub(crate) fn bump_skipped(&mut self) {
        while self
            .tokens
            .get(self.pos)
            .is_some_and(|token| is_skipped(token.kind))
        {
            self.bump_one();
        }
    }

    /// Emits the next token as `kind`, which the mode it was lexed in did not give it.
    /// The `:` before input and output types is one.
    pub(crate) fn bump_as(&mut self, kind: SyntaxKind) {
        self.bump_skipped();
        let token = self.nth_token(0).expect("bump_as at the end");
        self.pos += 1;
        self.push_token(kind, token.range);
    }

    pub(crate) fn bump(&mut self) {
        self.bump_skipped();
        assert!(self.pos < self.tokens.len(), "bump at the end");
        self.bump_one();
    }

    /// Emits the next token inside an `Error` node, reporting `kind` at it.
    pub(crate) fn bump_error(&mut self, kind: ParseErrorKind) {
        let token = self.nth_token(0).expect("bump_error at the end");
        self.bump_error_to(
            token.range.end(),
            ParseError {
                kind,
                range: token.range,
            },
        );
    }

    /// Emits the tokens ahead up to `end` inside an `Error` node, reporting `error`.
    pub(crate) fn bump_error_to(&mut self, end: TextSize, error: ParseError) {
        self.start(SyntaxKind::Error);
        self.push_error(error);
        while self
            .nth_token(0)
            .is_some_and(|token| token.range.end() <= end)
        {
            let token = self.tokens[self.pos];
            assert!(
                token.kind != SyntaxKind::Error,
                "bump_error_to over an Error token"
            );
            self.bump_one();
        }
        assert!(
            self.tokens[..self.pos]
                .last()
                .is_some_and(|token| token.range.end() == end),
            "bump_error_to past the end of a token"
        );
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

    fn bump_one(&mut self) {
        let token = self.tokens[self.pos];
        self.pos += 1;
        self.events.push(Event::Token(token));
        if token.kind == SyntaxKind::Error {
            let error = self.lex_errors.next().expect("an error per Error token");
            self.push_error(error.into());
        }
    }

    fn push_token(&mut self, kind: SyntaxKind, range: TextRange) {
        self.events.push(Event::Token(Token {
            kind,
            range,
        }));
    }

    fn push_error(&mut self, error: ParseError) {
        self.errors += 1;
        self.events.push(Event::Error(error));
    }
}

fn is_skipped(kind: SyntaxKind) -> bool {
    matches!(kind, SyntaxKind::Whitespace | SyntaxKind::Comment)
}

/// Whether a comment among `tokens` has no whitespace before its `#`.
/// `tokens` is the inside of a bracketed word of `src`.
/// The lexer reads such a `#` as no comment when it reads the word.
fn has_glued_comment(src: &str, tokens: &[Token]) -> bool {
    tokens
        .iter()
        .filter(|token| token.kind == SyntaxKind::Comment)
        .any(|comment| {
            let before = usize::from(comment.range.start()).checked_sub(1);
            !lexer::starts_comment_after(before.map(|at| src.as_bytes()[at]))
        })
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

    use super::{Event, Parser};
    use crate::{
        ParseErrorKind,
        grammar::SQUARE,
        lexer::{self, Mode, Token},
    };

    fn parser(src: &str) -> Parser<'_> {
        let lexed = lexer::lex(src, TextRange::up_to(TextSize::of(src)), Mode::Block);
        Parser::new(src, lexed, 0)
    }

    #[test]
    #[should_panic(expected = "bump_bracketed at another token")]
    fn bump_bracketed_emits_only_the_token_bracketed_looked_at() {
        let mut p = parser("[a] [b]");
        let first = p.bracketed(&SQUARE, Mode::List).unwrap();
        p.bump();
        p.bump_bracketed(SyntaxKind::List, first, |_| {});
    }

    #[test]
    #[should_panic(expected = "items left some of what the token holds")]
    fn bump_bracketed_needs_items_to_take_everything() {
        let mut p = parser("[a]");
        let bracketed = p.bracketed(&SQUARE, Mode::List).unwrap();
        p.bump_bracketed(SyntaxKind::List, bracketed, |_| {});
    }

    #[test]
    fn bump_bracketed_or_word_drops_items_that_report_an_error() {
        let mut p = parser(" [a]");
        let bracketed = p.bracketed(&SQUARE, Mode::List).unwrap();
        p.bump_bracketed_or_word(SyntaxKind::List, bracketed, |p| {
            p.bump_error(ParseErrorKind::ShellAndAnd);
        });
        assert_eq!(
            p.events,
            [
                Event::Token(Token {
                    kind: SyntaxKind::Whitespace,
                    range: TextRange::up_to(TextSize::new(1)),
                }),
                Event::Token(Token {
                    kind: SyntaxKind::Word,
                    range: TextRange::new(TextSize::new(1), TextSize::new(4)),
                }),
            ]
        );
        assert_eq!(p.errors, 0);
        assert_eq!(p.peek(), None);
    }

    #[test]
    fn bump_bracketed_keeps_a_word_holding_a_comment_with_no_whitespace_before_it() {
        for src in ["[a,#c]", "[a;#c\n]", "[a #c\n,#d]", "[#c\n]"] {
            let mut p = parser(src);
            let bracketed = p.bracketed(&SQUARE, Mode::List).unwrap();
            p.bump_bracketed(SyntaxKind::List, bracketed, |_| {});
            assert_eq!(
                p.events,
                [Event::Token(Token {
                    kind: SyntaxKind::Word,
                    range: TextRange::up_to(TextSize::of(src)),
                })],
                "{src:?}"
            );
        }
    }

    #[test]
    fn bump_bracketed_expands_a_word_holding_a_comment_after_whitespace() {
        for src in ["[a #c\n]", "[a\n#c\n]", "[ #c\n]"] {
            let mut p = parser(src);
            let bracketed = p.bracketed(&SQUARE, Mode::List).unwrap();
            p.bump_bracketed(SyntaxKind::List, bracketed, |p| {
                while p.peek().is_some() {
                    p.bump();
                }
            });
            assert_eq!(p.events[0], Event::Start(SyntaxKind::List), "{src:?}");
        }
    }

    #[test]
    fn bump_bracketed_or_word_keeps_the_node_over_an_error_inside_a_nested_node() {
        let mut p = parser("[[a]]");
        let outer = p.bracketed(&SQUARE, Mode::List).unwrap();
        p.bump_bracketed_or_word(SyntaxKind::List, outer, |p| {
            let inner = p.bracketed(&SQUARE, Mode::List).unwrap();
            p.bump_bracketed(SyntaxKind::List, inner, |p| {
                p.bump_error(ParseErrorKind::ShellAndAnd);
            });
        });
        let starts = p
            .events
            .iter()
            .filter(|event| matches!(event, Event::Start(SyntaxKind::List)))
            .count();
        assert_eq!(starts, 2);
        assert!(
            p.events
                .iter()
                .any(|event| matches!(event, Event::Error(_)))
        );
    }
}
