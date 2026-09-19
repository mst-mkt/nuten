//! Lexer and parser for Nushell.

mod grammar;
pub mod invariants;
mod lexer;
mod parser;
#[cfg(test)]
mod test_support;
mod tree;

use std::fmt;

use nuten_syntax::SyntaxNode;
use rowan::{GreenNode, TextRange, TextSize};

pub use crate::lexer::LexErrorKind;
use crate::lexer::{LexError, Mode};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Parsed {
    green: GreenNode,
    errors: Vec<ParseError>,
}

impl Parsed {
    pub fn syntax(&self) -> SyntaxNode {
        SyntaxNode::new_root(self.green.clone())
    }

    /// In the order they appear in the source.
    pub fn errors(&self) -> &[ParseError] {
        &self.errors
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ParseError {
    pub kind: ParseErrorKind,
    /// Always inside an `Error` element.
    pub range: TextRange,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParseErrorKind {
    Lex(LexErrorKind),
    /// `||`, which nu spells `or`.
    ShellOrOr,
    /// `&&`, which nu spells `and` or `;`.
    ShellAndAnd,
    /// `2>`, which nu spells `e>`.
    ShellErrRedirect,
    /// `2>&1`, which nu spells `o+e>`.
    ShellOutErrRedirect,
    /// `o>|` or `out>|`, which is the same as `|`.
    OutPipe,
    /// `;` after a plain `|`, however many line breaks and comments come between.
    SemicolonAfterPipe,
    /// A plain `|` as the last token of the file, past any comment lines right below it.
    /// A blank line or a line break at the very end makes nu accept it.
    PipeAtEof,
    /// A plain `|` in what an assignment operator takes, where a line break ends the statement.
    /// Other pipelines end there without an error.
    PipeEndsAssignment,
    /// A redirection with no command before it to apply to.
    RedirectionAtHead,
    /// A file redirection without a target item right after it on the same line.
    /// The operators of other shells do not count as one.
    /// Reported at the operator, where nu points at what follows it.
    MissingRedirectionTarget,
    /// A redirection of a stream the element already redirects.
    MultipleRedirections,
    /// A `:`, `=`, or `,` where the parameters of a `def` or an `extern` take a parameter or a flag, a `(-f)` after a `,`, or a `:` or an `=` with nothing at all after it.
    /// nu rejects each of these, and dropping a `,` would take the error away.
    SignatureSeparator,
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
            ParseErrorKind::PipeAtEof | ParseErrorKind::PipeEndsAssignment => {
                "no command after `|`"
            },
            ParseErrorKind::RedirectionAtHead => "redirection with no command before it",
            ParseErrorKind::MissingRedirectionTarget => "redirection with no target after it",
            ParseErrorKind::SignatureSeparator => {
                "`:`, `=`, or `,` with no parameter, type, or default value for it"
            },
            ParseErrorKind::MultipleRedirections => {
                "redirection of a stream that is already redirected"
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

/// The tree holds every byte of `src`, even when there are errors.
pub fn parse(src: &str) -> Parsed {
    let lexed = lexer::lex(src, TextRange::up_to(TextSize::of(src)), Mode::Block);
    tree::build(src, &parser::parse(src, lexed))
}

#[cfg(test)]
mod tests {
    use super::{LexErrorKind, ParseErrorKind};

    #[test]
    fn error_kinds_print_a_message() {
        assert_eq!(
            ParseErrorKind::Lex(LexErrorKind::Unclosed).to_string(),
            "unclosed quote or bracket"
        );
        assert_eq!(
            ParseErrorKind::PipeAtEof.to_string(),
            "no command after `|`"
        );
    }
}
