//! Parser that turns tokens into events for the tree builder.

use std::{iter, mem, rc::Rc, vec};

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
    /// Whether `text` starts with the opening bracket and ends with its closer.
    pub(crate) fn wrap(&self, text: &str) -> bool {
        let bytes = text.as_bytes();
        bytes.first() == Some(&self.open) && bytes.last() == Some(&self.close)
    }

    /// Whether the brackets wrap `range` of `src`, the opening one closing at its last byte.
    fn enclose(&self, src: &str, range: TextRange) -> bool {
        if !self.wrap(&src[range]) {
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
        let (_, inside, _) = split(self.range);
        Bracketed {
            inner: self.inner.nested(inside, mode, self.inner.depth),
            ..self
        }
    }
}

/// A piece of a token, as `Parser::split` cuts it.
#[derive(Debug, Clone, Copy)]
pub(crate) enum Piece {
    /// Text `split` lexes in block mode, taking it only where it makes one item with no error.
    Lexed(TextRange),
    /// Text emitted as a token of this kind.
    Token(SyntaxKind, TextRange),
}

/// What `Parser::relexed` or `Parser::split` found in the next token.
pub(crate) struct Relexed<'a> {
    inner: Parser<'a>,
    token: Token,
}

impl<'a> Relexed<'a> {
    /// A parser of the pieces of the token, to look into before emitting it.
    pub(crate) fn inner(&self) -> &Parser<'a> {
        &self.inner
    }
}

pub(crate) fn parse(src: &str, lexed: Lexed) -> Vec<Event> {
    let mut parser = Parser::new(
        src,
        lexed,
        Mode::Block.skips_newline(),
        grammar::alias_names(src),
        0,
    );
    // Not `start`: that would put leading trivia outside Root.
    parser.events.push(Event::Start(SyntaxKind::Root));
    grammar::file(&mut parser);
    parser.bump_skipped();
    parser.events.push(Event::Finish);
    parser.events
}

/// Whitespace and comments are skipped when looking ahead, as is a line break in some modes.
/// They are emitted just before the next token or node, or at the end of the tokens.
/// So no node but Root starts or ends with them.
pub(crate) struct Parser<'a> {
    src: &'a str,
    tokens: Vec<Token>,
    lex_errors: vec::IntoIter<LexError>,
    pos: usize,
    events: Vec<Event>,
    skips_newline: bool,
    /// The names the file aliases, shared with the `Parser` of every level of brackets.
    alias_names: Rc<[&'a str]>,
    /// Brackets around the tokens.
    depth: usize,
    /// How many errors were reported among these tokens.
    /// Those inside a bracketed word read as a node are not counted.
    /// Whether `{ }` reads as a block depends on its own statements alone.
    errors: usize,
}

impl<'a> Parser<'a> {
    fn new(
        src: &'a str,
        lexed: Lexed,
        skips_newline: bool,
        alias_names: Rc<[&'a str]>,
        depth: usize,
    ) -> Parser<'a> {
        Parser {
            src,
            tokens: lexed.tokens,
            lex_errors: lexed.errors.into_iter(),
            pos: 0,
            events: Vec::new(),
            skips_newline,
            alias_names,
            depth,
            errors: 0,
        }
    }

    pub(crate) fn is_aliased(&self, name: &str) -> bool {
        self.alias_names.binary_search(&name).is_ok()
    }

    pub(crate) fn is_skipped(&self, kind: SyntaxKind) -> bool {
        matches!(kind, SyntaxKind::Whitespace | SyntaxKind::Comment)
            || (kind == SyntaxKind::Newline && self.skips_newline)
    }

    /// `read` is told whether a line break was skipped before.
    pub(crate) fn with_newlines_read(&mut self, read: impl FnOnce(&mut Parser<'a>, bool)) {
        let skipped = mem::replace(&mut self.skips_newline, false);
        read(self, skipped);
        self.skips_newline = skipped;
    }

    pub(crate) fn peek(&self) -> Option<SyntaxKind> {
        self.nth(0)
    }

    pub(crate) fn peek_text(&self) -> Option<&'a str> {
        self.nth_text(0)
    }

    pub(crate) fn peek_range(&self) -> Option<TextRange> {
        self.nth_range(0)
    }

    /// The kind of the `n`th token ahead, `0` being the next, or `None` past the end.
    pub(crate) fn nth(&self, n: usize) -> Option<SyntaxKind> {
        self.nth_token(n).map(|token| token.kind)
    }

    /// The text of the `n`th token ahead, `0` being the next, or `None` past the end.
    pub(crate) fn nth_text(&self, n: usize) -> Option<&'a str> {
        self.nth_token(n).map(|token| &self.src[token.range])
    }

    /// The range of the `n`th token ahead, `0` being the next, or `None` past the end.
    pub(crate) fn nth_range(&self, n: usize) -> Option<TextRange> {
        self.nth_token(n).map(|token| token.range)
    }

    /// The kind of the last token emitted, trivia aside.
    pub(crate) fn last_kind(&self) -> Option<SyntaxKind> {
        self.tokens[..self.pos]
            .iter()
            .rev()
            .map(|token| token.kind)
            .find(|kind| !kind.is_trivia())
    }

    /// The kind of the last token emitted, the skipped ones aside.
    /// Unlike `last_kind`, it counts a line break the parser does not skip.
    pub(crate) fn last_unskipped_kind(&self) -> Option<SyntaxKind> {
        self.tokens[..self.pos]
            .iter()
            .rev()
            .map(|token| token.kind)
            .find(|&kind| !self.is_skipped(kind))
    }

    /// The kinds of every token ahead, trivia included.
    /// Only for looking past what `peek` sees.
    pub(crate) fn remaining_kinds(&self) -> impl Iterator<Item = SyntaxKind> {
        self.tokens[self.pos..].iter().map(|token| token.kind)
    }

    /// As `remaining_kinds`, with each token and its text.
    pub(crate) fn remaining_tokens(&self) -> impl Iterator<Item = (Token, &'a str)> + Clone {
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
        Some(Bracketed {
            inner: self.nested(inside, mode, self.depth + 1),
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
        self.read_inner(&mut inner, items);
        self.push_token(brackets.close_kind, close);
        self.finish();
        if let Affix::Suffix(kind) = affix {
            self.push_token(kind, TextRange::new(close.end(), token.range.end()));
        }
        inner.errors
    }

    /// The next token lexed again in `mode`, when it is a word.
    pub(crate) fn relexed(&self, mode: Mode) -> Option<Relexed<'a>> {
        let token = self
            .nth_token(0)
            .filter(|token| token.kind == SyntaxKind::Word)?;
        Some(Relexed {
            inner: self.nested(token.range, mode, self.depth),
            token,
        })
    }

    /// The next token cut into `pieces`, when it is a word and each lexed piece is one item.
    /// The pieces cover the token in order.
    pub(crate) fn split(&self, pieces: &[Piece]) -> Option<Relexed<'a>> {
        let token = self
            .nth_token(0)
            .filter(|token| token.kind == SyntaxKind::Word)?;
        let tokens = pieces
            .iter()
            .map(|&piece| match piece {
                Piece::Lexed(range) => self.lexed_item(range),
                Piece::Token(kind, range) => Some(Token {
                    kind,
                    range,
                }),
            })
            .collect::<Option<Vec<_>>>()?;
        let starts =
            iter::once(token.range.start()).chain(tokens.iter().map(|piece| piece.range.end()));
        assert!(
            tokens
                .iter()
                .zip(starts)
                .all(|(piece, start)| piece.range.start() == start)
                && tokens.last().map(|piece| piece.range.end()) == Some(token.range.end()),
            "pieces that do not cover the token"
        );
        Some(Relexed {
            inner: self.over(tokens),
            token,
        })
    }

    /// A parser of `range` of the source, when block mode lexes it as one item with no error.
    /// Its one token is the next, to look into as a piece of the token ahead.
    pub(crate) fn item_at(&self, range: TextRange) -> Option<Parser<'a>> {
        let item = self.lexed_item(range)?;
        Some(self.over(vec![item]))
    }

    fn lexed_item(&self, range: TextRange) -> Option<Token> {
        match lexer::lex(self.src, range, Mode::Block).tokens[..] {
            [item] if item.kind.is_item() && item.kind != SyntaxKind::Error => Some(item),
            _ => None,
        }
    }

    /// Emits the next token as a `node` of the events `items` makes of its pieces.
    /// Unlike what brackets hold, the pieces stand among these tokens, so their errors count here.
    pub(crate) fn bump_relexed(
        &mut self,
        node: SyntaxKind,
        relexed: Relexed<'a>,
        items: impl FnOnce(&mut Parser<'a>),
    ) {
        let Relexed {
            mut inner,
            token,
        } = relexed;
        assert_eq!(
            self.nth_token(0),
            Some(token),
            "bump_relexed at another token"
        );
        self.start(node);
        self.pos += 1;
        self.read_inner(&mut inner, items);
        self.errors += inner.errors;
        self.finish();
    }

    /// Lexes the tokens ahead up to `end` again in `mode`, and emits the events `items` makes.
    /// Their lexing errors give way to those of lexing them again.
    /// The tokens skipped before them are emitted as they are, since `mode` may read a line break.
    pub(crate) fn bump_relexed_to(
        &mut self,
        end: TextSize,
        mode: Mode,
        items: impl FnOnce(&mut Parser<'a>),
    ) {
        self.bump_skipped();
        let taken = self.take_to(end);
        let start = taken
            .tokens
            .first()
            .map_or(end, |token| token.range.start());
        let mut inner = self.nested(TextRange::new(start, end), mode, self.depth);
        self.read_inner(&mut inner, items);
        self.errors += inner.errors;
    }

    /// A parser of `range` of the source lexed again in `mode`.
    /// It shows what `bump_relexed_to` would emit, before it does.
    pub(crate) fn lexed_range(&self, range: TextRange, mode: Mode) -> Parser<'a> {
        self.nested(range, mode, self.depth)
    }

    /// Emits the tokens ahead up to `end` as the events `items` makes of them.
    /// `items` sees no token past `end`.
    pub(crate) fn bump_bounded(&mut self, end: TextSize, items: impl FnOnce(&mut Parser<'a>)) {
        let taken = self.take_to(end);
        let mut inner = Parser::new(
            self.src,
            taken,
            self.skips_newline,
            Rc::clone(&self.alias_names),
            self.depth,
        );
        self.read_inner(&mut inner, items);
        self.errors += inner.errors;
    }

    /// Moves past the tokens ahead up to `end`, and returns them with their lexing errors.
    fn take_to(&mut self, end: TextSize) -> Lexed {
        let count = self.tokens[self.pos..]
            .iter()
            .take_while(|token| token.range.end() <= end)
            .count();
        assert!(
            self.tokens
                .get(self.pos + count)
                .is_none_or(|token| token.range.start() >= end),
            "end inside a token"
        );
        let tokens = self.tokens[self.pos..self.pos + count].to_vec();
        self.pos += count;
        let errors = tokens
            .iter()
            .filter(|token| token.kind == SyntaxKind::Error)
            .count();
        Lexed {
            errors: self.lex_errors.by_ref().take(errors).collect(),
            tokens,
        }
    }

    /// A parser of `range` of the source lexed in `mode`, inside `depth` brackets.
    fn nested(&self, range: TextRange, mode: Mode, depth: usize) -> Parser<'a> {
        let lexed = lexer::lex(self.src, range, mode);
        Parser::new(
            self.src,
            lexed,
            mode.skips_newline(),
            Rc::clone(&self.alias_names),
            depth,
        )
    }

    /// A parser of `tokens`, pieces of the tokens here with no lexing error.
    fn over(&self, tokens: Vec<Token>) -> Parser<'a> {
        let lexed = Lexed {
            tokens,
            errors: Vec::new(),
        };
        Parser::new(
            self.src,
            lexed,
            Mode::Block.skips_newline(),
            Rc::clone(&self.alias_names),
            self.depth,
        )
    }

    /// Runs `items` to the end of `inner`, a parser of what this one moves past.
    /// The events are lent to it, so that nested nodes do not copy the events inside.
    fn read_inner(&mut self, inner: &mut Parser<'a>, items: impl FnOnce(&mut Parser<'a>)) {
        inner.events = mem::take(&mut self.events);
        items(inner);
        inner.bump_skipped();
        assert!(
            inner.pos == inner.tokens.len(),
            "items left some of what inner holds"
        );
        self.events = mem::take(&mut inner.events);
    }

    /// Emits the skipped tokens ahead, so that a node does not start or end with them.
    pub(crate) fn bump_skipped(&mut self) {
        while self
            .tokens
            .get(self.pos)
            .is_some_and(|token| self.is_skipped(token.kind))
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
            .filter(|token| !self.is_skipped(token.kind))
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

    use super::{Event, Parser, Piece};
    use crate::{
        ParseErrorKind, grammar,
        grammar::SQUARE,
        lexer::{self, Mode, Token},
    };

    fn parser(src: &str) -> Parser<'_> {
        let lexed = lexer::lex(src, TextRange::up_to(TextSize::of(src)), Mode::Block);
        Parser::new(
            src,
            lexed,
            Mode::Block.skips_newline(),
            grammar::alias_names(src),
            0,
        )
    }

    #[test]
    #[should_panic(expected = "bump_bracketed at another token")]
    fn bump_bracketed_emits_only_token_bracketed_looked_at() {
        let mut p = parser("[a] [b]");
        let first = p.bracketed(&SQUARE, Mode::List).unwrap();
        p.bump();
        p.bump_bracketed(SyntaxKind::List, first, |_| {});
    }

    #[test]
    #[should_panic(expected = "items left some of what inner holds")]
    fn bump_bracketed_needs_items_to_take_everything() {
        let mut p = parser("[a]");
        let bracketed = p.bracketed(&SQUARE, Mode::List).unwrap();
        p.bump_bracketed(SyntaxKind::List, bracketed, |_| {});
    }

    #[test]
    #[should_panic(expected = "bump_relexed at another token")]
    fn bump_relexed_emits_only_token_relexed_looked_at() {
        let mut p = parser("$a.b $c.d");
        let first = p.relexed(Mode::CellPath).unwrap();
        p.bump();
        p.bump_relexed(SyntaxKind::CellPath, first, |_| {});
    }

    #[test]
    #[should_panic(expected = "items left some of what inner holds")]
    fn bump_relexed_needs_items_to_take_everything() {
        let mut p = parser("$a.b");
        let relexed = p.relexed(Mode::CellPath).unwrap();
        p.bump_relexed(SyntaxKind::CellPath, relexed, |_| {});
    }

    #[test]
    fn bump_relexed_counts_errors_among_pieces_as_its_own() {
        let mut p = parser("$a?");
        let relexed = p.relexed(Mode::CellPath).unwrap();
        p.bump_relexed(SyntaxKind::CellPath, relexed, |p| {
            p.bump();
            p.bump_error(ParseErrorKind::ShellAndAnd);
        });
        assert_eq!(p.errors, 1);
    }

    #[test]
    fn relexed_takes_only_word() {
        for src in ["\"a.b\"", "e>", ""] {
            assert!(parser(src).relexed(Mode::CellPath).is_none(), "{src:?}");
        }
    }

    fn range(start: u32, end: u32) -> TextRange {
        TextRange::new(start.into(), end.into())
    }

    #[test]
    fn split_cuts_word_into_pieces() {
        let p = parser("$a..(b c)");
        let split = p
            .split(&[
                Piece::Lexed(range(0, 2)),
                Piece::Token(SyntaxKind::RangeOperator, range(2, 4)),
                Piece::Lexed(range(4, 9)),
            ])
            .unwrap();
        let pieces: Vec<_> = split
            .inner()
            .remaining_tokens()
            .map(|(token, text)| (token.kind, text))
            .collect();
        assert_eq!(
            pieces,
            [
                (SyntaxKind::Word, "$a"),
                (SyntaxKind::RangeOperator, ".."),
                (SyntaxKind::Word, "(b c)"),
            ]
        );
    }

    #[test]
    fn split_takes_only_lexed_piece_of_one_item() {
        let word = |start, end| Piece::Token(SyntaxKind::Word, range(start, end));
        for (src, pieces) in [
            ("a#b", [word(0, 1), Piece::Lexed(range(1, 3))]),
            ("a\"b", [word(0, 1), Piece::Lexed(range(1, 3))]),
            ("[a b]", [Piece::Lexed(range(0, 4)), word(4, 5)]),
            ("(a b)", [word(0, 1), Piece::Lexed(range(1, 5))]),
        ] {
            assert!(parser(src).split(&pieces).is_none(), "{src:?}");
        }
    }

    #[test]
    fn split_takes_only_word() {
        let p = parser("\"a\"");
        assert!(p.split(&[Piece::Lexed(range(0, 3))]).is_none());
    }

    #[test]
    #[should_panic(expected = "pieces that do not cover the token")]
    fn split_panics_on_pieces_that_do_not_cover_token() {
        parser("ab").split(&[Piece::Lexed(range(0, 1))]);
    }

    #[test]
    fn item_at_takes_only_one_item_with_no_error() {
        let p = parser("a b \"c");
        assert_eq!(p.item_at(range(0, 1)).unwrap().peek_text(), Some("a"));
        for piece in [range(0, 3), range(1, 2), range(4, 6)] {
            assert!(p.item_at(piece).is_none(), "{piece:?}");
        }
    }

    #[test]
    fn nth_range_counts_tokens_past_whitespace_and_comments() {
        let p = parser("a  # c\n  b");
        assert_eq!(p.nth_range(0), Some(range(0, 1)));
        assert_eq!(p.nth_range(1), Some(range(6, 7)));
        assert_eq!(p.nth_range(2), Some(range(9, 10)));
        assert_eq!(p.nth_range(3), None);
    }

    fn bump_all(p: &mut Parser<'_>) {
        while p.peek().is_some() {
            p.bump();
        }
    }

    #[test]
    fn bump_relexed_to_emits_tokens_of_lexing_again() {
        let mut p = parser("a<b c");
        p.bump_relexed_to(TextSize::new(3), Mode::Signature, bump_all);
        assert_eq!(p.peek_text(), Some("c"));
        assert_eq!(p.errors, 1);
        assert!(matches!(p.events[1], Event::Error(_)));
    }

    #[test]
    fn bump_relexed_to_emits_skipped_tokens_before_relexed_ones_as_they_are() {
        let src = "\na";
        let lexed = lexer::lex(src, TextRange::up_to(TextSize::of(src)), Mode::Paren);
        let mut p = Parser::new(
            src,
            lexed,
            Mode::Paren.skips_newline(),
            grammar::alias_names(src),
            0,
        );
        p.bump_relexed_to(TextSize::of(src), Mode::VarType, bump_all);
        assert_eq!(
            p.events,
            [
                Event::Token(Token {
                    kind: SyntaxKind::Newline,
                    range: range(0, 1),
                }),
                Event::Token(Token {
                    kind: SyntaxKind::Word,
                    range: range(1, 2),
                }),
            ]
        );
    }

    #[test]
    fn bump_relexed_to_drops_lexing_errors_of_tokens_it_replaces() {
        let mut p = parser("a) b)");
        p.bump_relexed_to(TextSize::new(2), Mode::Signature, bump_all);
        p.bump();
        let errors: Vec<_> = p
            .events
            .iter()
            .filter_map(|event| match event {
                Event::Error(error) => Some(error.range),
                _ => None,
            })
            .collect();
        assert_eq!(errors, [range(1, 2), range(4, 5)]);
    }

    #[test]
    fn bump_bounded_hides_tokens_past_end() {
        let mut p = parser("a b) c");
        p.bump_bounded(TextSize::new(4), |p| {
            bump_all(p);
            assert_eq!(p.peek(), None);
        });
        assert_eq!(p.peek_text(), Some("c"));
        assert_eq!(p.errors, 1);
    }

    #[test]
    #[should_panic(expected = "end inside a token")]
    fn bump_relexed_to_panics_at_end_inside_token() {
        parser("ab").bump_relexed_to(TextSize::new(1), Mode::Signature, bump_all);
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
