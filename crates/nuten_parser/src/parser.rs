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

pub(crate) fn parse(lexed: Lexed) -> Vec<Event> {
    let mut parser = Parser {
        tokens: lexed.tokens,
        lex_errors: lexed.errors.into_iter(),
        pos: 0,
        events: Vec::new(),
    };
    grammar::root(&mut parser);
    parser.events
}

pub(crate) struct Parser {
    tokens: Vec<Token>,
    lex_errors: vec::IntoIter<LexError>,
    pos: usize,
    events: Vec<Event>,
}

impl Parser {
    pub(crate) fn at_end(&self) -> bool {
        self.pos == self.tokens.len()
    }

    pub(crate) fn bump(&mut self) {
        let token = self.tokens[self.pos];
        self.pos += 1;
        self.events.push(Event::Token(token));
        if token.kind == SyntaxKind::Error {
            let error = self.lex_errors.next().expect("an error per Error token");
            self.events.push(Event::Error(error.into()));
        }
    }

    pub(crate) fn start(&mut self, kind: SyntaxKind) {
        self.events.push(Event::Start(kind));
    }

    pub(crate) fn finish(&mut self) {
        self.events.push(Event::Finish);
    }
}
