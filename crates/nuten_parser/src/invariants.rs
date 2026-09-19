//! What the lexer and the parser must keep, asked of any input, with syntax
//! errors or without.

use std::{fmt, iter, ops::Range};

use nuten_syntax::{
    SyntaxKind, SyntaxNode,
    ast::{AstNode, Node},
};
use rowan::{TextRange, TextSize};

use crate::{
    ParseError, Parsed,
    lexer::{self, LexError, LexErrorKind, Lexed, Mode, Token},
};

/// One property of the lexer or the parser.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Invariant {
    /// The tokens cover the source exactly: they leave no gap and nothing
    /// over, none is empty, and none starts inside a char.
    TokenCoverage,
    /// No token that nu reads as one is split in two: no `Whitespace` follows
    /// another, and no `\r\n` straddles a boundary.
    TokenSplits,
    /// The text of every token is one the lexer may give that kind.
    TokenShape,
    /// The lexing errors match the `Error` tokens one to one, in order, each
    /// inside its token and over what the error is about.
    LexErrorPlacement,
    /// The tree restores the source byte for byte.
    Roundtrip,
    /// Every node but Root holds text, and its first and last tokens are not
    /// trivia.
    NodeBounds,
    /// Every node has a node kind and the shape `nuten_syntax::ast` reads: a
    /// bracketed node opens and closes with its brackets, a block holds its
    /// parameters right after its brace, and a table holds its column names
    /// and their `;` first.
    NodeShape,
    /// The errors match the `Error` elements one to one, in order, each
    /// inside its element.
    ParseErrorPlacement,
    /// Every error of lexing the source is among the errors, so no error is
    /// lost when a token is split.
    LexErrorsReported,
}

impl Invariant {
    /// The name in reports.
    pub fn name(self) -> &'static str {
        match self {
            Invariant::TokenCoverage => "token coverage",
            Invariant::TokenSplits => "token splits",
            Invariant::TokenShape => "token shape",
            Invariant::LexErrorPlacement => "lex error placement",
            Invariant::Roundtrip => "roundtrip",
            Invariant::NodeBounds => "node bounds",
            Invariant::NodeShape => "node shape",
            Invariant::ParseErrorPlacement => "parse error placement",
            Invariant::LexErrorsReported => "lex errors reported",
        }
    }

    /// A violation of this invariant, given what breaks in one line.
    fn violation(self, detail: String) -> Violation {
        Violation {
            invariant: self.name(),
            detail,
        }
    }
}

/// An invariant that does not hold for one input. The invariant is named
/// rather than typed, so that one list can hold the violations of the parser
/// and of the formatter, which declare their own.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Violation {
    /// The name of the invariant, as the layer that declares it reports it.
    pub invariant: &'static str,
    /// What breaks first, in one line.
    pub detail: String,
}

impl Violation {
    /// The same, with the lexer mode it was found in at the end of the detail.
    fn in_mode(self, mode: Mode) -> Violation {
        Violation {
            invariant: self.invariant,
            detail: format!("{} in {mode:?}", self.detail),
        }
    }
}

/// One line.
impl fmt::Display for Violation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.invariant, self.detail)
    }
}

/// Every spelling of each operator kind, written out apart from how the lexer
/// reads them, so that one checks the other.
pub(crate) const OPERATORS: &[(SyntaxKind, &[&str])] = &[
    (SyntaxKind::Assign, &["=", "+=", "++=", "-=", "*=", "/="]),
    (SyntaxKind::OutGt, &["o>", "out>"]),
    (SyntaxKind::OutGtGt, &["o>>", "out>>"]),
    (SyntaxKind::ErrGt, &["e>", "err>"]),
    (SyntaxKind::ErrGtGt, &["e>>", "err>>"]),
    (
        SyntaxKind::OutErrGt,
        &["o+e>", "e+o>", "out+err>", "err+out>"],
    ),
    (
        SyntaxKind::OutErrGtGt,
        &["o+e>>", "e+o>>", "out+err>>", "err+out>>"],
    ),
    (SyntaxKind::ErrGtPipe, &["e>|", "err>|"]),
    (
        SyntaxKind::OutErrGtPipe,
        &["o+e>|", "e+o>|", "out+err>|", "err+out>|"],
    ),
];

/// Every violation of the lexer for all of `src` in every mode, mode by mode
/// and in the order of the invariants within one, each detail ending with
/// its mode. Tokens that do not cover the source are reported on their own.
pub fn lexer_violations(src: &str) -> Vec<Violation> {
    Mode::ALL
        .iter()
        .flat_map(|&mode| {
            let lexed = lexer::lex(src, TextRange::up_to(TextSize::of(src)), mode);
            lexed_violations(src, &lexed, mode)
                .into_iter()
                .map(move |violation| violation.in_mode(mode))
        })
        .collect()
}

/// Every violation of `lexed`, the lexing of all of `src` in `mode`.
fn lexed_violations(src: &str, lexed: &Lexed, mode: Mode) -> Vec<Violation> {
    // `token_splits` and `token_shape` read the text a token stands for;
    // covering the source is what leaves it there to read.
    if let Some(violation) = token_coverage(src, &lexed.tokens) {
        return vec![violation];
    }
    [
        token_splits(src, &lexed.tokens),
        token_shape(src, &lexed.tokens, mode),
        lex_error_placement(src, lexed),
    ]
    .into_iter()
    .flatten()
    .collect()
}

fn token_coverage(src: &str, tokens: &[Token]) -> Option<Violation> {
    // `is_char_boundary` is false of a position past the end of `src` too,
    // so `start_inside_char` comes after the check that names that flaw.
    let detail = wrong_range(tokens)
        .or_else(|| wrong_end(src, tokens))
        .or_else(|| start_inside_char(src, tokens))?;
    Some(Invariant::TokenCoverage.violation(detail))
}

fn wrong_range(tokens: &[Token]) -> Option<String> {
    let ends = iter::once(TextSize::new(0)).chain(tokens.iter().map(|token| token.range.end()));
    tokens.iter().zip(ends).find_map(|(token, end)| {
        let flaw = if token.range.start() != end {
            format!("does not start at {end:?}")
        } else if token.range.is_empty() {
            "is empty".to_string()
        } else {
            return None;
        };
        Some(format!("{:?} at {:?} {flaw}", token.kind, token.range))
    })
}

fn wrong_end(src: &str, tokens: &[Token]) -> Option<String> {
    let source_end = TextSize::of(src);
    let end = tokens
        .last()
        .map_or(TextSize::new(0), |token| token.range.end());
    (end != source_end)
        .then(|| format!("the tokens end at {end:?}, but the source ends at {source_end:?}"))
}

fn start_inside_char(src: &str, tokens: &[Token]) -> Option<String> {
    tokens.iter().find_map(|token| {
        let start = usize::from(token.range.start());
        (!src.is_char_boundary(start))
            .then(|| format!("{:?} at {:?} starts inside a char", token.kind, token.range))
    })
}

fn token_splits(src: &str, tokens: &[Token]) -> Option<Violation> {
    let detail = tokens.windows(2).find_map(|pair| {
        let (left, right) = (pair[0], pair[1]);
        let split = if left.kind == SyntaxKind::Whitespace && right.kind == SyntaxKind::Whitespace {
            "whitespace"
        } else if src[left.range].ends_with('\r') && src[right.range].starts_with('\n') {
            r"\r\n"
        } else {
            return None;
        };
        Some(format!("a {split} is split at {:?}", right.range.start()))
    })?;
    Some(Invariant::TokenSplits.violation(detail))
}

fn token_shape(src: &str, tokens: &[Token], mode: Mode) -> Option<Violation> {
    let detail = tokens.iter().find_map(|token| {
        let text = &src[token.range];
        (!well_formed(token.kind, text, mode))
            .then(|| format!("{:?} at {:?} is {text:?}", token.kind, token.range))
    })?;
    Some(Invariant::TokenShape.violation(detail))
}

/// Whether the lexer may give `kind` to `text` in `mode`.
fn well_formed(kind: SyntaxKind, text: &str, mode: Mode) -> bool {
    let quoted = |text: &str| {
        text.len() >= 2 && text.starts_with(['\'', '"', '`']) && text.ends_with(&text[..1])
    };
    match kind {
        SyntaxKind::Whitespace => text.bytes().all(|b| matches!(b, b' ' | b'\t' | b'\r')),
        SyntaxKind::Newline => text == "\n" || text == "\r\n",
        SyntaxKind::Comment => text.starts_with('#') && !text.contains('\n'),
        SyntaxKind::Pipe => text == "|",
        SyntaxKind::PipePipe => text == "||",
        SyntaxKind::Semicolon => text == ";",
        SyntaxKind::String => quoted(text),
        SyntaxKind::InterpString => text
            .strip_prefix('$')
            .is_some_and(|rest| rest.starts_with(['"', '\'']) && quoted(rest)),
        SyntaxKind::RawString => text.starts_with("r#") && text.ends_with('#'),
        SyntaxKind::Word | SyntaxKind::Error => {
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

fn lex_error_placement(src: &str, lexed: &Lexed) -> Option<Violation> {
    let error_tokens: Vec<_> = lexed
        .tokens
        .iter()
        .copied()
        .filter(|token| token.kind == SyntaxKind::Error)
        .collect();
    let detail = if error_tokens.len() == lexed.errors.len() {
        let (index, (token, error)) = error_tokens
            .iter()
            .copied()
            .zip(lexed.errors.iter().copied())
            .enumerate()
            .find(|&(_, (token, error))| !well_placed(src, error, token))?;
        format!(
            "error {} at {:?} is misplaced in its Error token at {:?}",
            index + 1,
            error.range,
            token.range
        )
    } else {
        format!(
            "Error tokens and errors differ in number: {} and {}",
            error_tokens.len(),
            lexed.errors.len()
        )
    };
    Some(Invariant::LexErrorPlacement.violation(detail))
}

/// Whether `error` is inside `token` and over what the error is about.
fn well_placed(src: &str, error: LexError, token: Token) -> bool {
    if !token.range.contains_range(error.range) {
        return false;
    }
    // The tokens covering the source says nothing of where the errors fall,
    // so a range `src` cannot be cut on is misplaced rather than read.
    let Some(text) = src.get(Range::<usize>::from(error.range)) else {
        return false;
    };
    let after = &src[usize::from(token.range.end())..];
    let raw_prefix = |text: &str| {
        text.strip_prefix('r')
            .is_some_and(|hashes| !hashes.is_empty() && hashes.bytes().all(|b| b == b'#'))
    };
    let ends_token = error.range.end() == token.range.end();
    match error.kind {
        // Runs to the end, or before a final `\` in a `"` string.
        LexErrorKind::Unclosed => {
            (matches!(
                text,
                "[" | "{" | "(" | "<" | "'" | "\"" | "`" | "$'" | "$\""
            ) || text.strip_suffix('\'').is_some_and(raw_prefix))
                && (after.is_empty() || after == "\\" && text.ends_with('"'))
        },
        LexErrorKind::UnmatchedCloser => matches!(text, "]" | "}" | ")") && ends_token,
        LexErrorKind::MissingRawQuote => raw_prefix(text) && ends_token,
    }
}

/// Every violation of the parser for `src`, in the order of the invariants.
/// `parsed` is the tree of `src`.
pub fn violations(src: &str, parsed: &Parsed) -> Vec<Violation> {
    let tree = parsed.syntax();
    [
        roundtrip(src, &tree),
        node_bounds(&tree),
        node_shape(&tree),
        parse_error_placement(&tree, parsed.errors()),
        lex_errors_reported(src, parsed.errors()),
    ]
    .into_iter()
    .flatten()
    .collect()
}

fn roundtrip(src: &str, tree: &SyntaxNode) -> Option<Violation> {
    if tree.text() == src {
        return None;
    }
    let text = tree.text().to_string();
    let offset = src
        .bytes()
        .zip(text.bytes())
        .position(|(a, b)| a != b)
        .unwrap_or(src.len().min(text.len()));
    Some(Invariant::Roundtrip.violation(format!(
        "the tree differs from the source from byte {offset}"
    )))
}

fn node_bounds(tree: &SyntaxNode) -> Option<Violation> {
    tree.descendants().skip(1).find_map(|node| {
        // A node has no first or last token when an empty node starts or
        // ends it, which is reported in turn.
        let flaw = if node.text_range().is_empty() {
            "is empty"
        } else if node.first_token()?.kind().is_trivia() {
            "starts with trivia"
        } else if node.last_token()?.kind().is_trivia() {
            "ends with trivia"
        } else {
            return None;
        };
        Some(Invariant::NodeBounds.violation(format!(
            "{:?} at {:?} {flaw}",
            node.kind(),
            node.text_range()
        )))
    })
}

fn node_shape(tree: &SyntaxNode) -> Option<Violation> {
    tree.descendants().find_map(|node| {
        let flaw = match Node::cast(node.clone()) {
            None => "has a token kind",
            Some(typed) => typed.flaw()?,
        };
        Some(Invariant::NodeShape.violation(format!(
            "{:?} at {:?} {flaw}",
            node.kind(),
            node.text_range()
        )))
    })
}

fn parse_error_placement(tree: &SyntaxNode, errors: &[ParseError]) -> Option<Violation> {
    let ranges: Vec<_> = tree
        .descendants_with_tokens()
        .filter(|element| element.kind() == SyntaxKind::Error)
        .map(|element| element.text_range())
        .collect();
    let detail = if ranges.len() == errors.len() {
        let (index, (range, error)) = ranges
            .iter()
            .zip(errors)
            .enumerate()
            .find(|(_, (range, error))| !range.contains_range(error.range))?;
        format!(
            "error {} at {:?} is outside its Error element at {range:?}",
            index + 1,
            error.range
        )
    } else {
        format!(
            "Error elements and errors differ in number: {} and {}",
            ranges.len(),
            errors.len()
        )
    };
    Some(Invariant::ParseErrorPlacement.violation(detail))
}

fn lex_errors_reported(src: &str, errors: &[ParseError]) -> Option<Violation> {
    let lexed = lexer::lex(src, TextRange::up_to(TextSize::of(src)), Mode::block());
    let missing = lexed
        .errors
        .into_iter()
        .map(ParseError::from)
        .find(|error| !errors.contains(error))?;
    Some(Invariant::LexErrorsReported.violation(format!(
        "{} at {:?} is not reported",
        missing.kind, missing.range
    )))
}

#[cfg(test)]
mod tests {
    use nuten_syntax::SyntaxKind::{
        self, Block, Call, Error, LBrace, LBracket, Newline, Pipe, Pipeline, RBracket, Root, Table,
        Whitespace, Word,
    };
    use rowan::{GreenNodeBuilder, TextRange};

    use super::{
        Invariant, Violation, lex_error_placement, lexed_violations, lexer_violations,
        token_coverage, token_shape, token_splits, violations,
    };
    use crate::{
        ParseError, ParseErrorKind, Parsed,
        lexer::{LexError, LexErrorKind, Lexed, Mode, Token},
        parse,
        test_support::short_inputs,
    };

    fn token(kind: SyntaxKind, start: u32, end: u32) -> Token {
        Token {
            kind,
            range: TextRange::new(start.into(), end.into()),
        }
    }

    fn lex_error(kind: LexErrorKind, start: u32, end: u32) -> LexError {
        LexError {
            kind,
            range: TextRange::new(start.into(), end.into()),
        }
    }

    /// Root with one node of `kind` that holds `tokens`.
    fn one_node(
        kind: SyntaxKind,
        tokens: &[(SyntaxKind, &str)],
        errors: Vec<ParseError>,
    ) -> Parsed {
        let mut builder = GreenNodeBuilder::new();
        builder.start_node(Root.to_raw());
        builder.start_node(kind.to_raw());
        for &(kind, text) in tokens {
            builder.token(kind.to_raw(), text);
        }
        builder.finish_node();
        builder.finish_node();
        Parsed {
            green: builder.finish(),
            errors,
        }
    }

    fn violation(invariant: Invariant, detail: &str) -> Violation {
        invariant.violation(detail.to_string())
    }

    #[test]
    fn lexer_invariants_hold_for_all_short_inputs() {
        for src in short_inputs() {
            let violations = lexer_violations(&src);
            assert!(violations.is_empty(), "{src:?}: {violations:?}");
        }
    }

    #[test]
    fn the_tokens_must_cover_the_source() {
        for (src, tokens, detail) in [
            (
                "ls",
                vec![token(Word, 1, 2)],
                "Word at 1..2 does not start at 0",
            ),
            (
                "ls",
                vec![token(Word, 0, 1), token(Word, 2, 2)],
                "Word at 2..2 does not start at 1",
            ),
            (
                "ls",
                vec![token(Word, 0, 2), token(Word, 1, 2)],
                "Word at 1..2 does not start at 2",
            ),
            (
                "ls",
                vec![token(Word, 0, 0), token(Word, 0, 2)],
                "Word at 0..0 is empty",
            ),
            // The token is reported rather than the end, which is wrong too.
            ("ls", vec![token(Word, 0, 0)], "Word at 0..0 is empty"),
            (
                "日",
                vec![token(Word, 0, 1), token(Word, 1, 3)],
                "Word at 1..3 starts inside a char",
            ),
            (
                "ls",
                vec![token(Word, 0, 1)],
                "the tokens end at 1, but the source ends at 2",
            ),
            (
                "ls",
                vec![token(Word, 0, 3)],
                "the tokens end at 3, but the source ends at 2",
            ),
            // The end is reported rather than the start being inside a char,
            // which `is_char_boundary` also says of a position past the end.
            (
                "ls",
                vec![token(Word, 0, 3), token(Word, 3, 5)],
                "the tokens end at 5, but the source ends at 2",
            ),
            (
                "ls",
                vec![],
                "the tokens end at 0, but the source ends at 2",
            ),
        ] {
            assert_eq!(
                token_coverage(src, &tokens),
                Some(violation(Invariant::TokenCoverage, detail)),
                "{src:?} {tokens:?}"
            );
        }
    }

    #[test]
    fn nothing_else_is_asked_where_the_tokens_do_not_cover_the_source() {
        // The `Newline` holds `"ls"`, so asking the shape of these tokens
        // would give a second violation, and `well_formed` would take the
        // first byte of the empty one.
        let lexed = Lexed {
            tokens: vec![token(Word, 0, 0), token(Newline, 0, 2)],
            errors: vec![],
        };
        assert_eq!(
            lexed_violations("ls", &lexed, Mode::block()),
            [violation(Invariant::TokenCoverage, "Word at 0..0 is empty")]
        );
    }

    #[test]
    fn a_token_nu_reads_as_one_must_not_be_split() {
        assert_eq!(
            token_splits("  ", &[token(Whitespace, 0, 1), token(Whitespace, 1, 2)]),
            Some(violation(
                Invariant::TokenSplits,
                "a whitespace is split at 1"
            ))
        );
        assert_eq!(
            token_splits("\r\n", &[token(Whitespace, 0, 1), token(Newline, 1, 2)]),
            Some(violation(Invariant::TokenSplits, r"a \r\n is split at 1"))
        );
    }

    #[test]
    fn the_text_of_a_token_must_fit_its_kind() {
        assert_eq!(
            token_shape("ls", &[token(Newline, 0, 2)], Mode::block()),
            Some(violation(
                Invariant::TokenShape,
                "Newline at 0..2 is \"ls\""
            ))
        );
        assert_eq!(
            token_shape(",", &[token(Word, 0, 1)], Mode::list()),
            Some(violation(Invariant::TokenShape, "Word at 0..1 is \",\""))
        );
        // The same token is well formed where `,` is not special.
        assert_eq!(token_shape(",", &[token(Word, 0, 1)], Mode::block()), None);
    }

    #[test]
    fn lexing_errors_must_match_error_tokens() {
        let lexed = Lexed {
            tokens: vec![token(Error, 0, 2)],
            errors: vec![],
        };
        assert_eq!(
            lex_error_placement("\"a", &lexed),
            Some(violation(
                Invariant::LexErrorPlacement,
                "Error tokens and errors differ in number: 1 and 0"
            ))
        );

        // The error must cover the opener, not the whole token.
        let lexed = Lexed {
            tokens: vec![token(Error, 0, 2)],
            errors: vec![lex_error(LexErrorKind::Unclosed, 0, 2)],
        };
        assert_eq!(
            lex_error_placement("\"a", &lexed),
            Some(violation(
                Invariant::LexErrorPlacement,
                "error 1 at 0..2 is misplaced in its Error token at 0..2"
            ))
        );

        // An error that splits a char is misplaced rather than read.
        let lexed = Lexed {
            tokens: vec![token(Error, 0, 3)],
            errors: vec![lex_error(LexErrorKind::Unclosed, 0, 1)],
        };
        assert_eq!(
            lex_error_placement("日", &lexed),
            Some(violation(
                Invariant::LexErrorPlacement,
                "error 1 at 0..1 is misplaced in its Error token at 0..3"
            ))
        );
    }

    #[test]
    fn invariants_hold_for_all_short_inputs() {
        for src in short_inputs() {
            let violations = violations(&src, &parse(&src));
            assert!(violations.is_empty(), "{src:?}: {violations:?}");
        }
    }

    #[test]
    fn the_tree_must_restore_the_source() {
        assert_eq!(
            violations("ls | first", &parse("ls | get")),
            [violation(
                Invariant::Roundtrip,
                "the tree differs from the source from byte 5"
            )]
        );
        assert_eq!(
            violations("ls", &parse("ls |")),
            [violation(
                Invariant::Roundtrip,
                "the tree differs from the source from byte 2"
            )]
        );
    }

    #[test]
    fn a_node_must_have_a_token_that_is_not_trivia_at_each_end() {
        for (parsed, detail) in [
            (one_node(Pipeline, &[], vec![]), "Pipeline at 0..0 is empty"),
            (
                one_node(Pipeline, &[(Whitespace, " "), (Word, "ls")], vec![]),
                "Pipeline at 0..3 starts with trivia",
            ),
            (
                one_node(Call, &[(Word, "ls"), (Newline, "\n")], vec![]),
                "Call at 0..3 ends with trivia",
            ),
        ] {
            let src = parsed.syntax().text().to_string();
            assert_eq!(
                violations(&src, &parsed),
                [violation(Invariant::NodeBounds, detail)]
            );
        }
    }

    #[test]
    fn an_empty_node_is_reported_rather_than_its_parent() {
        for (call_at, detail) in [(0, "Call at 0..0 is empty"), (1, "Call at 2..2 is empty")] {
            let mut builder = GreenNodeBuilder::new();
            builder.start_node(Root.to_raw());
            builder.start_node(Pipeline.to_raw());
            for index in 0..2 {
                if index == call_at {
                    builder.start_node(Call.to_raw());
                    builder.finish_node();
                } else {
                    builder.token(Word.to_raw(), "ls");
                }
            }
            builder.finish_node();
            builder.finish_node();
            let parsed = Parsed {
                green: builder.finish(),
                errors: vec![],
            };
            assert_eq!(
                violations("ls", &parsed),
                [violation(Invariant::NodeBounds, detail)]
            );
        }
    }

    #[test]
    fn a_node_must_have_the_shape_ast_reads() {
        for (parsed, detail) in [
            (
                one_node(Word, &[(Word, "ls")], vec![]),
                "Word at 0..2 has a token kind",
            ),
            (
                one_node(Block, &[(LBrace, "{"), (Word, "ls}")], vec![]),
                "Block at 0..4 does not close with its bracket",
            ),
            (
                one_node(
                    Table,
                    &[(LBracket, "["), (Word, "[a]"), (RBracket, "]")],
                    vec![],
                ),
                "Table at 0..5 does not hold its column names and their `;` first",
            ),
        ] {
            let src = parsed.syntax().text().to_string();
            assert_eq!(
                violations(&src, &parsed),
                [violation(Invariant::NodeShape, detail)]
            );
        }
    }

    #[test]
    fn errors_must_match_error_elements() {
        let parsed = one_node(Error, &[(Pipe, "|")], vec![]);
        assert_eq!(
            violations("|", &parsed),
            [violation(
                Invariant::ParseErrorPlacement,
                "Error elements and errors differ in number: 1 and 0"
            )]
        );

        let outside = ParseError {
            kind: ParseErrorKind::PipeAtEof,
            range: TextRange::new(2.into(), 3.into()),
        };
        let parsed = one_node(Error, &[(Word, "ls")], vec![outside]);
        assert_eq!(
            violations("ls", &parsed),
            [violation(
                Invariant::ParseErrorPlacement,
                "error 1 at 2..3 is outside its Error element at 0..2"
            )]
        );
    }

    #[test]
    fn every_lexing_error_must_be_reported() {
        let parse_error =
            |kind, start: u32, end: u32| ParseError::from(lex_error(kind, start, end));
        for (kind, errors) in [
            (Word, vec![]),
            (Error, vec![parse_error(LexErrorKind::Unclosed, 1, 2)]),
            (
                Error,
                vec![parse_error(LexErrorKind::UnmatchedCloser, 0, 1)],
            ),
        ] {
            let parsed = one_node(Pipeline, &[(kind, "\"a")], errors);
            assert_eq!(
                violations("\"a", &parsed),
                [violation(
                    Invariant::LexErrorsReported,
                    "unclosed quote or bracket at 0..1 is not reported"
                )]
            );
        }
    }

    #[test]
    fn a_violation_prints_in_one_line() {
        assert_eq!(
            violations("ls | first", &parse("ls | get"))[0].to_string(),
            "roundtrip: the tree differs from the source from byte 5"
        );
    }
}
