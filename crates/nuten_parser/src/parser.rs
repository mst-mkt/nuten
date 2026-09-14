//! Parser that turns tokens into events for the tree builder.

use std::vec;

use nuten_syntax::SyntaxKind;

use crate::{
    ParseError,
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
    root(&mut parser);
    parser.events
}

struct Parser {
    tokens: Vec<Token>,
    lex_errors: vec::IntoIter<LexError>,
    pos: usize,
    events: Vec<Event>,
}

impl Parser {
    fn at_end(&self) -> bool {
        self.pos == self.tokens.len()
    }

    fn bump(&mut self) {
        let token = self.tokens[self.pos];
        self.pos += 1;
        self.events.push(Event::Token(token));
        if token.kind == SyntaxKind::Error {
            let error = self.lex_errors.next().expect("an error per Error token");
            self.events.push(Event::Error(error.into()));
        }
    }

    fn start(&mut self, kind: SyntaxKind) {
        self.events.push(Event::Start(kind));
    }

    fn finish(&mut self) {
        self.events.push(Event::Finish);
    }
}

fn root(p: &mut Parser) {
    p.start(SyntaxKind::Root);
    while !p.at_end() {
        p.bump();
    }
    p.finish();
}
