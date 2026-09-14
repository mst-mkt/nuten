//! Parser that turns tokens into events for the tree builder.

use std::vec;

use nuten_syntax::SyntaxKind;

use crate::{
    ParseError, grammar,
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
        self.next_token().map(|token| token.kind)
    }

    /// The text of the next token, or `None` at the end.
    pub(crate) fn peek_text(&self) -> Option<&'a str> {
        self.next_token().map(|token| &self.src[token.range])
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

    pub(crate) fn start(&mut self, kind: SyntaxKind) {
        self.bump_skipped();
        self.events.push(Event::Start(kind));
    }

    pub(crate) fn finish(&mut self) {
        self.events.push(Event::Finish);
    }

    fn next_token(&self) -> Option<Token> {
        self.tokens[self.pos..]
            .iter()
            .copied()
            .find(|token| !is_skipped(token.kind))
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
