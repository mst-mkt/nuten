mod grammar;
pub mod invariants;
mod lexer;
mod parser;
#[cfg(test)]
mod test_support;
mod tree;

use std::fmt;

use nuten_syntax::{SyntaxElement, SyntaxKind, SyntaxNode, SyntaxToken};
use rowan::{GreenNode, TextRange, TextSize};

use crate::lexer::{LexError, Mode};
pub use crate::{
    grammar::{is_command, operator_precedence},
    lexer::LexErrorKind,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Parsed {
    green: GreenNode,
    errors: Vec<ParseError>,
}

impl Parsed {
    pub fn syntax(&self) -> SyntaxNode {
        SyntaxNode::new_root(self.green.clone())
    }

    pub fn errors(&self) -> &[ParseError] {
        &self.errors
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ParseError {
    pub kind: ParseErrorKind,
    pub range: TextRange,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParseErrorKind {
    Lex(LexErrorKind),
    ShellOrOr,
    ShellAndAnd,
    ShellErrRedirect,
    ShellOutErrRedirect,
    OutPipe,
    SemicolonAfterPipe,
    PipeAtEof,
    /// Only after an assignment operator, since a line break ends any other pipeline silently.
    PipeEndsAssignment,
    PipeEndsList,
    RedirectionAtHead,
    MissingRedirectionTarget,
    MultipleRedirections,
    MissingOperator,
    UnknownOperator {
        spelling: &'static str,
        instead: &'static str,
    },
    MissingTerm,
    /// Reported where nu rejects it, since dropping a `,` would take the error away.
    SignatureSeparator,
    MissingVariableName,
    InvalidVariableName,
    RecordPatternKey,
    /// Nu reads no more of the path after it.
    CellPathSeparator {
        expected: &'static str,
    },
    NegativeCellPathIndex,
    CellPathMember,
    BinaryDigits,
    SpreadInTable,
    AttributeWithoutDefinition,
}

impl fmt::Display for ParseErrorKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self {
            ParseErrorKind::Lex(kind) => return fmt::Display::fmt(kind, f),
            ParseErrorKind::ShellOrOr => {
                "`||` is not an operator; use `or` (`try` for the shell meaning)"
            },
            ParseErrorKind::ShellAndAnd => {
                "`&&` is not an operator; use `and` (`;` for the shell meaning)"
            },
            ParseErrorKind::ShellErrRedirect => "`2>` is not a redirection; use `e>`",
            ParseErrorKind::ShellOutErrRedirect => "`2>&1` is not a redirection; use `o+e>`",
            ParseErrorKind::OutPipe => "redirecting stdout to a pipe is just `|`",
            ParseErrorKind::SemicolonAfterPipe => "no command between `|` and `;`",
            ParseErrorKind::PipeAtEof
            | ParseErrorKind::PipeEndsAssignment
            | ParseErrorKind::PipeEndsList => "no command after `|`",
            ParseErrorKind::RedirectionAtHead => "redirection with no command before it",
            ParseErrorKind::MissingRedirectionTarget => "redirection with no target after it",
            ParseErrorKind::MissingOperator => "term with no operator before it",
            ParseErrorKind::UnknownOperator {
                spelling,
                instead,
            } => return write!(f, "`{spelling}` is not an operator; use {instead}"),
            ParseErrorKind::MissingTerm => "operator with no term after it",
            ParseErrorKind::SignatureSeparator => {
                "`:`, `=`, or `,` with no parameter, type, or default value for it"
            },
            ParseErrorKind::MultipleRedirections => {
                "redirection of a stream that is already redirected"
            },
            ParseErrorKind::MissingVariableName => "`$` with no variable name after it",
            ParseErrorKind::InvalidVariableName => {
                "variable name with a character no variable name takes"
            },
            ParseErrorKind::RecordPatternKey => {
                "key of a record pattern with no `:` and pattern after it"
            },
            ParseErrorKind::CellPathSeparator {
                expected,
            } => return write!(f, "token where a cell path takes {expected}"),
            ParseErrorKind::NegativeCellPathIndex => "negative index in a cell path",
            ParseErrorKind::CellPathMember => "member of a cell path that is no string",
            ParseErrorKind::BinaryDigits => "binary whose items do not decode to bytes",
            ParseErrorKind::SpreadInTable => "spread in a table",
            ParseErrorKind::AttributeWithoutDefinition => {
                "attribute with no `def` or `extern` after it"
            },
        };
        f.write_str(message)
    }
}

impl From<LexError> for ParseError {
    fn from(error: LexError) -> Self {
        Self {
            kind: ParseErrorKind::Lex(error.kind),
            range: error.range,
        }
    }
}

pub fn parse(src: &str) -> Parsed {
    let lexed = lexer::lex(src, TextRange::up_to(TextSize::of(src)), Mode::Block);
    tree::build(src, parser::parse(src, lexed))
}

/// Nu starts a comment at a `#` inside a word too, after a byte that is whitespace in Latin-1.
/// In any bracket around the word, that comment runs on to the end of the line.
/// `root` is the tree of `src`.
pub fn tokens_hiding_line_end<'a>(
    src: &'a str,
    root: &SyntaxNode,
) -> impl Iterator<Item = SyntaxToken> + 'a {
    // Most files hold no such `#`, and the text tells that sooner than the tree.
    (0..src.len())
        .any(|at| lexer::opens_comment_in_word(src.as_bytes(), at))
        .then(|| root.descendants_with_tokens())
        .into_iter()
        .flatten()
        .filter_map(SyntaxElement::into_token)
        .filter(|token| hides_line_end(src, token))
}

/// Strings and comments hold no comment of their own.
/// A comment that ends at a line break inside the token hides nothing after it.
fn hides_line_end(src: &str, token: &SyntaxToken) -> bool {
    if matches!(
        token.kind(),
        SyntaxKind::Comment
            | SyntaxKind::String
            | SyntaxKind::RawString
            | SyntaxKind::InterpString
            | SyntaxKind::InterpText
    ) {
        return false;
    }
    let start = usize::from(token.text_range().start());
    let text = token.text();
    text.match_indices('#').any(|(at, _)| {
        lexer::opens_comment_in_word(src.as_bytes(), start + at)
            && !text[at..].contains(['\n', '\r'])
    })
}

#[cfg(test)]
mod tests {
    use super::{LexErrorKind, ParseErrorKind};

    #[test]
    fn error_kinds_print_message() {
        assert_eq!(
            ParseErrorKind::Lex(LexErrorKind::Unclosed).to_string(),
            "unclosed quote or bracket"
        );
        assert_eq!(
            ParseErrorKind::PipeAtEof.to_string(),
            "no command after `|`"
        );
        assert_eq!(
            ParseErrorKind::UnknownOperator {
                spelling: "%",
                instead: "`mod`",
            }
            .to_string(),
            "`%` is not an operator; use `mod`"
        );
        assert_eq!(
            ParseErrorKind::CellPathSeparator {
                expected: "`.`"
            }
            .to_string(),
            "token where a cell path takes `.`"
        );
    }
}
