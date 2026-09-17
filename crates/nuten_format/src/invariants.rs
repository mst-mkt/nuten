//! What formatting must keep, asked of any input.
//!
//! A fixture has an expected output; any other input has only these: that
//! the output keeps every token and every comment, and that formatting the
//! output changes nothing. What the parser must keep is in
//! `nuten_parser::invariants`.

use std::fmt;

use nuten_parser::{Parsed, parse};
use nuten_syntax::{SyntaxElement, SyntaxKind, SyntaxNode, SyntaxToken};

use crate::{Config, FormatError, format_parsed};

/// One property of the formatter.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Invariant {
    /// The output has the tokens of the source, trivia aside, in order.
    /// Line endings inside a token do not count, since the printer rewrites
    /// them, nor do `Comma` tokens, which separate the items of lists and
    /// records as whitespace does. A long flag of a signature and the short
    /// flag after it count as one, since the formatter joins them, as nu
    /// reads them.
    Tokens,
    /// The output has the comments of the source, in order, each on its own
    /// line or at the end of a line as it was, and, between statements, with
    /// a blank line after it or not as it was. Spaces and tabs at the end of
    /// a comment do not count, since the printer trims them, nor do blank
    /// lines at the end of the file or a block, which the formatter drops, nor
    /// `Comma` tokens, which it may drop too.
    Comments,
    /// Formatting the output succeeds and changes nothing.
    Idempotency,
}

impl Invariant {
    /// The name in reports.
    pub fn name(self) -> &'static str {
        match self {
            Invariant::Tokens => "token preservation",
            Invariant::Comments => "comment preservation",
            Invariant::Idempotency => "idempotency",
        }
    }
}

/// An invariant that does not hold for one input.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Violation {
    pub invariant: Invariant,
    /// What differs, in one line.
    pub detail: String,
}

impl Violation {
    fn new(invariant: Invariant, detail: String) -> Violation {
        Violation {
            invariant,
            detail,
        }
    }
}

/// One line.
impl fmt::Display for Violation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.invariant.name(), self.detail)
    }
}

/// Every violation for `src`, in the order of the invariants.
///
/// # Errors
///
/// When the source has syntax errors.
///
/// # Panics
///
/// When the parser breaks its invariants on `src`, since the rules rely on
/// them.
pub fn violations(src: &str, config: &Config) -> Result<Vec<Violation>, FormatError> {
    let parsed = parse(src);
    let broken = nuten_parser::invariants::violations(src, &parsed);
    assert!(
        broken.is_empty(),
        "the parser breaks its invariants: {broken:?}"
    );
    let output = format_parsed(src, &parsed, config)?;
    Ok(against_output(&parsed.syntax(), &output, config))
}

/// The invariants of the formatter, asked of the tree of the source against
/// the output.
fn against_output(tree: &SyntaxNode, output: &str, config: &Config) -> Vec<Violation> {
    let parsed = parse(output);
    if let Some(error) = parsed.errors().first() {
        // An Error token can swallow several tokens, so the other checks
        // would only repeat this.
        return vec![Violation::new(
            Invariant::Idempotency,
            format!(
                "the output does not parse: {} at {:?}",
                error.kind, &output[error.range]
            ),
        )];
    }
    let output_tree = parsed.syntax();
    let mut violations = Vec::new();
    violations.extend(tokens(tree, &output_tree));
    violations.extend(comments(tree, &output_tree));
    violations.extend(idempotency(output, &parsed, config));
    violations
}

fn tokens(tree: &SyntaxNode, output_tree: &SyntaxNode) -> Option<Violation> {
    // The leaves rather than the lexer's tokens, since the parser relabels
    // some.
    fn leaves(tree: &SyntaxNode) -> Vec<(SyntaxKind, String)> {
        let mut leaves: Vec<(SyntaxKind, String)> = Vec::new();
        let tokens = tree
            .descendants_with_tokens()
            .filter_map(SyntaxElement::into_token)
            .filter(|token| !token.kind().is_trivia() && token.kind() != SyntaxKind::Comma);
        for token in tokens {
            let text = token.text().replace("\r\n", "\n");
            match leaves.last_mut() {
                Some((_, last)) if joins_flag(&token, last, &text) => last.push_str(&text),
                _ => leaves.push((token.kind(), text)),
            }
        }
        leaves
    }
    /// Whether the formatter joins `short`, a parameter of a signature, to
    /// the long flag `long` before it, as nu reads the two.
    fn joins_flag(short: &SyntaxToken, long: &str, text: &str) -> bool {
        short
            .parent()
            .is_some_and(|param| param.kind() == SyntaxKind::Param)
            && crate::rules::joins_flag(long, text)
    }
    fn describe((kind, text): &(SyntaxKind, String)) -> String {
        format!("{kind:?} {text:?}")
    }
    let (source, output) = (leaves(tree), leaves(output_tree));
    let index = mismatch(&source, &output);
    let detail = match (source.get(index), output.get(index)) {
        (None, None) => return None,
        (Some(a), Some(b)) => format!(
            "token {} is {} in the source and {} in the output",
            index + 1,
            describe(a),
            describe(b)
        ),
        (Some(a), None) => format!(
            "token {}, {}, is missing from the output",
            index + 1,
            describe(a)
        ),
        (None, Some(b)) => format!("token {}, {}, is not in the source", index + 1, describe(b)),
    };
    Some(Violation::new(Invariant::Tokens, detail))
}

fn comments(tree: &SyntaxNode, output_tree: &SyntaxNode) -> Option<Violation> {
    let (source, output) = (comments_of(tree), comments_of(output_tree));
    let index = mismatch(&source, &output);
    let detail = match (source.get(index), output.get(index)) {
        (None, None) => return None,
        (Some(a), Some(b)) => format!(
            "comment {} is {a} in the source and {b} in the output",
            index + 1
        ),
        (Some(a), None) => format!("comment {}, {a}, is missing from the output", index + 1),
        (None, Some(b)) => format!("comment {}, {b}, is not in the source", index + 1),
    };
    Some(Violation::new(Invariant::Comments, detail))
}

/// A comment and where it stands: on its own line or not, before a blank
/// line or not.
#[derive(Debug, PartialEq, Eq)]
struct Comment {
    /// Without the spaces and tabs at its end.
    text: String,
    own_line: bool,
    /// False inside a statement and at the end of the file or a block.
    blank_line_after: bool,
}

impl fmt::Display for Comment {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?} ", self.text)?;
        f.write_str(if self.own_line {
            "on its own line"
        } else {
            "at the end of a line"
        })?;
        if self.blank_line_after {
            f.write_str(" with a blank line after it")?;
        }
        Ok(())
    }
}

fn comments_of(tree: &SyntaxNode) -> Vec<Comment> {
    let tokens: Vec<_> = tree
        .descendants_with_tokens()
        .filter_map(SyntaxElement::into_token)
        .filter(|token| !matches!(token.kind(), SyntaxKind::Whitespace | SyntaxKind::Comma))
        .collect();
    tokens
        .iter()
        .enumerate()
        .filter(|(_, token)| token.kind() == SyntaxKind::Comment)
        .map(|(index, token)| {
            let own_line = index == 0 || tokens[index - 1].kind() == SyntaxKind::Newline;
            // Inside a statement the formatter drops blank lines, and nu
            // reads no documentation there.
            let between_statements = token.parent().is_some_and(|parent| {
                matches!(parent.kind(), SyntaxKind::Root | SyntaxKind::Block)
            });
            let mut after = tokens[index + 1..].iter().map(SyntaxToken::kind);
            let blank_line_after = between_statements
                && after.next() == Some(SyntaxKind::Newline)
                && after.next() == Some(SyntaxKind::Newline)
                && after
                    .find(|&kind| kind != SyntaxKind::Newline)
                    .is_some_and(|kind| kind != SyntaxKind::RBrace);
            Comment {
                text: token.text().trim_end_matches([' ', '\t']).to_string(),
                own_line,
                blank_line_after,
            }
        })
        .collect()
}

/// `parsed` is the tree of `output`, without errors.
fn idempotency(output: &str, parsed: &Parsed, config: &Config) -> Option<Violation> {
    let Ok(again) = format_parsed(output, parsed, config) else {
        unreachable!("the output parsed without errors")
    };
    if again == output {
        return None;
    }
    let (once, twice): (Vec<_>, Vec<_>) = (output.lines().collect(), again.lines().collect());
    let index = mismatch(&once, &twice);
    let detail = match (once.get(index), twice.get(index)) {
        (Some(once), Some(twice)) => format!(
            "line {} of the output, {once:?}, becomes {twice:?} when formatted again",
            index + 1
        ),
        (Some(once), None) => format!(
            "line {} of the output, {once:?}, is dropped when formatted again",
            index + 1
        ),
        (None, Some(twice)) => format!(
            "line {}, {twice:?}, is added to the output when formatted again",
            index + 1
        ),
        // `lines` hides the line endings, so they can change without any
        // line changing.
        (None, None) => format!(
            "the output differs from byte {} when formatted again",
            mismatch(output.as_bytes(), again.as_bytes())
        ),
    };
    Some(Violation::new(Invariant::Idempotency, detail))
}

/// The index of the first element that differs, or the length of the
/// shorter one.
fn mismatch<T: PartialEq>(a: &[T], b: &[T]) -> usize {
    a.iter()
        .zip(b)
        .position(|(a, b)| a != b)
        .unwrap_or(a.len().min(b.len()))
}

#[cfg(test)]
mod tests {
    use nuten_parser::parse;

    use super::{Invariant, Violation, against_output, violations};
    use crate::{Config, LineEnding, ParseErrorKind};

    /// The violations of the formatter's invariants for `output` as the
    /// output of `src`.
    fn check(src: &str, output: &str) -> Vec<Violation> {
        against_output(&parse(src).syntax(), output, &Config::default())
    }

    fn invariants(violations: &[Violation]) -> Vec<Invariant> {
        violations.iter().map(|v| v.invariant).collect()
    }

    #[test]
    fn ordinary_files_have_no_violations() {
        for src in [
            "# a\n\nls | get name # b\n# c\nls\n",
            "# c",
            "ls # c",
            "# a\nls # b\n",
            "# a\n# b\n",
            "#\n",
            "#   \n",
            "ls |\n# c\n\n| first\n",
            "ls\r\n# c\r\n\r\nls\r\n",
        ] {
            assert_eq!(violations(src, &Config::default()).unwrap(), [], "{src:?}");
        }
        let crlf = Config {
            line_ending: LineEnding::CrLf,
            ..Config::default()
        };
        assert_eq!(violations("ls\r\n# c\r\n\r\nls\r\n", &crlf).unwrap(), []);
    }

    #[test]
    fn a_file_with_syntax_errors_has_no_output_to_ask_about() {
        let error = violations("ls |", &Config::default()).unwrap_err();
        assert_eq!(error.errors[0].kind, ParseErrorKind::PipeAtEof);
    }

    #[test]
    fn the_output_must_keep_every_token() {
        let violations = check("ls | get name\n", "ls | first\n");
        assert_eq!(invariants(&violations), [Invariant::Tokens]);
        assert_eq!(
            violations[0].detail,
            "token 3 is Word \"get\" in the source and Word \"first\" in the output"
        );

        let violations = check("ls | get name\n", "ls | get\n");
        assert_eq!(invariants(&violations), [Invariant::Tokens]);
        assert_eq!(
            violations[0].detail,
            "token 4, Word \"name\", is missing from the output"
        );
        let violations = check("ls | get\n", "ls | get name\n");
        assert_eq!(invariants(&violations), [Invariant::Tokens]);
        assert_eq!(
            violations[0].detail,
            "token 4, Word \"name\", is not in the source"
        );
    }

    #[test]
    fn the_output_may_move_tokens_between_lines() {
        assert_eq!(check("ls |\n  get name\n", "ls | get name\n"), []);
    }

    #[test]
    fn line_endings_inside_a_token_do_not_count() {
        assert_eq!(check("'a\r\nb'\r\n", "'a\nb'\n"), []);
    }

    #[test]
    fn commas_that_are_tokens_do_not_count() {
        assert_eq!(check("[a, b,]\n", "[a b]\n"), []);
        assert_eq!(
            check("[a\n, # c\nb]\n", "[\n    a\n    # c\n    b\n]\n"),
            []
        );
        for (src, output) in [("echo , a\n", "echo a\n"), ("echo a,b\n", "echo ab\n")] {
            assert_eq!(
                invariants(&check(src, output)),
                [Invariant::Tokens],
                "{src:?}"
            );
        }
    }

    #[test]
    fn the_output_must_keep_every_comment_where_it_stands() {
        assert_eq!(
            invariants(&check("ls # c\n", "ls\n")),
            [Invariant::Comments]
        );
        let violations = check("ls # c\n", "# c\nls\n");
        assert_eq!(invariants(&violations), [Invariant::Comments]);
        assert_eq!(
            violations[0].detail,
            "comment 1 is \"# c\" at the end of a line in the source \
             and \"# c\" on its own line in the output"
        );
        let violations = check("# c\n\nls\n", "# c\nls\n");
        assert_eq!(invariants(&violations), [Invariant::Comments]);
        assert_eq!(
            violations[0].detail,
            "comment 1 is \"# c\" on its own line with a blank line after it in the source \
             and \"# c\" on its own line in the output"
        );
        let violations = check("# c\nls\n", "# c\n\nls\n");
        assert_eq!(invariants(&violations), [Invariant::Comments]);
        let violations = check("ls\n", "ls # c\n");
        assert_eq!(invariants(&violations), [Invariant::Comments]);
        assert_eq!(
            violations[0].detail,
            "comment 1, \"# c\" at the end of a line, is not in the source"
        );
    }

    #[test]
    fn trailing_whitespace_and_blank_lines_at_the_end_do_not_count() {
        assert_eq!(check("ls # c  \n", "ls # c\n"), []);
        assert_eq!(check("# c\n\n\n", "# c\n"), []);
        assert_eq!(check("# a\n\n\n# b\n", "# a\n\n# b\n"), []);
        assert_eq!(
            check(
                "echo {\nls\n# c\n\n}\nls\n",
                "echo {\n    ls\n    # c\n}\nls\n"
            ),
            []
        );
    }

    #[test]
    fn blank_lines_between_statements_of_a_block_count() {
        let violations = check("echo {\n# c\n\nls\n}\n", "echo {\n    # c\n    ls\n}\n");
        assert_eq!(invariants(&violations), [Invariant::Comments]);
    }

    #[test]
    fn blank_lines_inside_a_statement_do_not_count() {
        assert_eq!(check("ls |\n# c\n\n| first\n", "ls |\n# c\n| first\n"), []);
    }

    #[test]
    fn the_output_must_not_change_when_formatted_again() {
        let violations = check("ls|first\n", "ls |  first\n");
        assert_eq!(invariants(&violations), [Invariant::Idempotency]);
        assert_eq!(
            violations[0].detail,
            "line 1 of the output, \"ls |  first\", becomes \"ls | first\" when formatted again"
        );
        let violations = check("ls\nls\n", "ls\n\n\nls\n");
        assert_eq!(invariants(&violations), [Invariant::Idempotency]);
        assert_eq!(
            violations[0].detail,
            "line 3 of the output, \"\", becomes \"ls\" when formatted again"
        );
        let violations = check("ls\n", "ls\n\n");
        assert_eq!(invariants(&violations), [Invariant::Idempotency]);
        assert_eq!(
            violations[0].detail,
            "line 2 of the output, \"\", is dropped when formatted again"
        );
    }

    #[test]
    fn a_change_of_line_ending_is_reported_by_byte() {
        let violations = check("ls\n", "ls\r\n");
        assert_eq!(invariants(&violations), [Invariant::Idempotency]);
        assert_eq!(
            violations[0].detail,
            "the output differs from byte 2 when formatted again"
        );
    }

    #[test]
    fn the_output_must_parse() {
        let violations = check("ls\n", "ls |");
        assert_eq!(invariants(&violations), [Invariant::Idempotency]);
        assert_eq!(
            violations[0].detail,
            "the output does not parse: no command after `|` at \"|\""
        );
    }

    #[test]
    fn a_violation_prints_in_one_line() {
        let violation = &check("ls\n", "ls # c\n")[0];
        assert_eq!(
            violation.to_string(),
            "comment preservation: comment 1, \"# c\" at the end of a line, is not in the source"
        );
    }
}
