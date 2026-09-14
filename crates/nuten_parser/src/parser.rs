//! Parser that turns tokens into events for the tree builder.

use std::vec;

use nuten_syntax::SyntaxKind;

use crate::{
    ParseError, ParseErrorKind, grammar,
    lexer::{LexError, Lexed, Token},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Event {
    Start(SyntaxKind),
    Token(Token),
    Error(ParseError),
    Finish,
}

pub(crate) fn parse(src: &str, lexed: Lexed) -> Vec<Event> {
    let mut parser = Parser {
        src,
        tokens: lexed.tokens,
        lex_errors: lexed.errors.into_iter(),
        pos: 0,
        events: Vec::new(),
    };
    // Not `start`: that would put leading trivia outside Root.
    parser.events.push(Event::Start(SyntaxKind::Root));
    grammar::file(&mut parser);
    parser.bump_skipped();
    parser.events.push(Event::Finish);
    parser.events
}

/// Whitespace and comments are skipped when looking ahead and emitted
/// just before the next token or node, or at the end of the file,
/// so no node but Root starts or ends with them.
/// Newlines are not skipped, since they end statements.
pub(crate) struct Parser<'a> {
    src: &'a str,
    tokens: Vec<Token>,
    lex_errors: vec::IntoIter<LexError>,
    pos: usize,
    events: Vec<Event>,
}

impl<'a> Parser<'a> {
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
    /// Only for deciding whether a line continues; everything else looks
    /// through `peek`.
    pub(crate) fn remaining_kinds(&self) -> impl Iterator<Item = SyntaxKind> {
        self.tokens[self.pos..].iter().map(|token| token.kind)
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
}

fn is_skipped(kind: SyntaxKind) -> bool {
    matches!(kind, SyntaxKind::Whitespace | SyntaxKind::Comment)
}
