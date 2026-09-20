//! What formatting must keep, asked of any input.
//!
//! A fixture has an expected output; any other input has only these.
//! The output keeps every token and every comment, and formatting it again changes nothing.
//! What the parser must keep is in `nuten_parser::invariants`, and `check` asks those first.

use std::fmt;

pub use nuten_parser::invariants::Violation;
use nuten_parser::{Parsed, parse};
use nuten_syntax::{SyntaxElement, SyntaxKind, SyntaxNode, SyntaxToken};

use crate::{Config, ParseError, format_parsed};

/// One property of the formatter.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Invariant {
    /// The output has the tokens of the source, trivia aside, in order.
    /// Line endings inside a token do not count, since the printer rewrites them.
    /// Nor do `Comma` tokens, which separate items as whitespace does in lists, records and types.
    /// So a `,` the formatter prints in a type without one in the source goes unseen here.
    /// Only the nu oracle tells whether it went where nu reads it as a separator.
    /// A `ShortFlag` counts as one with the token before it.
    /// The formatter may join the two, as nu reads them.
    Tokens,
    /// The output has the comments of the source, in order.
    /// Each stands on its own line or at the end of a line, as it was.
    /// Between statements or the arms of a `match`, a blank line follows it or not, as it was.
    /// Spaces and tabs at the end of a comment do not count, since the printer trims them.
    /// Nor do blank lines at the end of the file, a block, or the arms, which the formatter drops.
    /// Nor do `Comma` tokens, which it may drop too.
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

    /// A violation of this invariant, given what breaks in one line.
    fn violation(self, detail: String) -> Violation {
        Violation {
            invariant: self.name(),
            detail,
        }
    }
}

/// What asking the invariants of one source finds.
/// The syntax errors are kept apart from the violations, since a source can have both.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Checked {
    /// What the parser breaks.
    /// Or what the formatter breaks, where the parser holds and the source has no syntax errors.
    pub violations: Vec<Violation>,
    /// The syntax errors of the source, which leave no output to ask about.
    pub syntax_errors: Vec<ParseError>,
}

/// What asking the invariants of `src` finds, with the violations in their order.
pub fn check(src: &str, config: &Config) -> Checked {
    let parsed = parse(src);
    Checked {
        violations: violations(src, &parsed, config),
        syntax_errors: parsed.errors().to_vec(),
    }
}

/// What `check` puts in `Checked::violations`.
/// `parsed` is the tree of `src`.
fn violations(src: &str, parsed: &Parsed, config: &Config) -> Vec<Violation> {
    // The rules read a tree that keeps the invariants of the parser.
    // What the formatter makes of one that does not says nothing.
    let broken = nuten_parser::invariants::violations(src, parsed);
    if !broken.is_empty() {
        return broken;
    }
    // `format_parsed` fails only on syntax errors, which leave no output.
    // `Checked` reports them on their own.
    let Ok(output) = format_parsed(src, parsed, config) else {
        return Vec::new();
    };
    against_output(&parsed.syntax(), &output, config)
}

/// The invariants of the formatter, asked of the tree of the source against the output.
fn against_output(tree: &SyntaxNode, output: &str, config: &Config) -> Vec<Violation> {
    let parsed = parse(output);
    if let Some(error) = parsed.errors().first() {
        // An Error token can swallow several tokens, so the other checks would only repeat this.
        return vec![Invariant::Idempotency.violation(format!(
            "the output does not parse: {} at {:?}",
            error.kind, &output[error.range]
        ))];
    }
    let output_tree = parsed.syntax();
    [
        tokens(tree, &output_tree),
        comments(tree, &output_tree),
        idempotency(output, &parsed, config),
    ]
    .into_iter()
    .flatten()
    .collect()
}

fn tokens(tree: &SyntaxNode, output_tree: &SyntaxNode) -> Option<Violation> {
    // The leaves rather than the lexer's tokens, since the parser relabels some.
    fn leaves(tree: &SyntaxNode) -> Vec<(SyntaxKind, String)> {
        let mut leaves: Vec<(SyntaxKind, String)> = Vec::new();
        let tokens = tree
            .descendants_with_tokens()
            .filter_map(SyntaxElement::into_token)
            .filter(|token| !token.kind().is_trivia() && token.kind() != SyntaxKind::Comma);
        for token in tokens {
            let text = token.text().replace("\r\n", "\n");
            match leaves.last_mut() {
                Some((_, last)) if token.kind() == SyntaxKind::ShortFlag => last.push_str(&text),
                _ => leaves.push((token.kind(), text)),
            }
        }
        leaves
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
    Some(Invariant::Tokens.violation(detail))
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
    Some(Invariant::Comments.violation(detail))
}

/// A comment and where it stands: on its own line or not, before a blank line or not.
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
            // Inside a statement the formatter drops blank lines.
            // Nu reads no documentation there either.
            let between_statements = token.parent().is_some_and(|parent| {
                matches!(
                    parent.kind(),
                    SyntaxKind::Root | SyntaxKind::Block | SyntaxKind::MatchArms
                )
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
        // `lines` hides the line endings, so they can change without any line changing.
        (None, None) => format!(
            "the output differs from byte {} when formatted again",
            mismatch(output.as_bytes(), again.as_bytes())
        ),
    };
    Some(Invariant::Idempotency.violation(detail))
}

/// The index of the first element that differs, or the length of the shorter one.
fn mismatch<T: PartialEq>(a: &[T], b: &[T]) -> usize {
    a.iter()
        .zip(b)
        .position(|(a, b)| a != b)
        .unwrap_or(a.len().min(b.len()))
}

#[cfg(test)]
mod tests {
    use nuten_parser::parse;

    use super::{Checked, Invariant, Violation, against_output, check};
    use crate::{Config, LineEnding, ParseErrorKind};

    /// The violations of the formatter's invariants for `output` as the output of `src`.
    fn formatter_violations(src: &str, output: &str) -> Vec<Violation> {
        against_output(&parse(src).syntax(), output, &Config::default())
    }

    fn invariant_names(violations: &[Violation]) -> Vec<&'static str> {
        violations
            .iter()
            .map(|violation| violation.invariant)
            .collect()
    }

    /// Neither a violation nor a syntax error.
    /// The syntax errors count too.
    /// A source that has them leaves the violations of the formatter unasked, and so empty.
    fn nothing() -> Checked {
        Checked {
            violations: vec![],
            syntax_errors: vec![],
        }
    }

    #[test]
    fn ordinary_files_leave_nothing_to_report() {
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
            assert_eq!(check(src, &Config::default()), nothing(), "{src:?}");
        }
        let crlf = Config {
            line_ending: LineEnding::CrLf,
            ..Config::default()
        };
        assert_eq!(check("ls\r\n# c\r\n\r\nls\r\n", &crlf), nothing());
    }

    #[test]
    fn a_file_with_syntax_errors_has_no_output_to_ask_about() {
        let checked = check("ls |", &Config::default());
        assert_eq!(checked.syntax_errors[0].kind, ParseErrorKind::PipeAtEof);
        assert_eq!(checked.violations, []);
    }

    #[test]
    fn the_output_must_keep_every_token() {
        let violations = formatter_violations("ls | get name\n", "ls | first\n");
        assert_eq!(invariant_names(&violations), [Invariant::Tokens.name()]);
        assert_eq!(
            violations[0].detail,
            "token 3 is Word \"get\" in the source and Word \"first\" in the output"
        );

        let violations = formatter_violations("ls | get name\n", "ls | get\n");
        assert_eq!(invariant_names(&violations), [Invariant::Tokens.name()]);
        assert_eq!(
            violations[0].detail,
            "token 4, Word \"name\", is missing from the output"
        );
        let violations = formatter_violations("ls | get\n", "ls | get name\n");
        assert_eq!(invariant_names(&violations), [Invariant::Tokens.name()]);
        assert_eq!(
            violations[0].detail,
            "token 4, Word \"name\", is not in the source"
        );
    }

    #[test]
    fn the_output_may_move_tokens_between_lines() {
        assert_eq!(
            formatter_violations("ls |\n  get name\n", "ls | get name\n"),
            []
        );
    }

    #[test]
    fn line_endings_inside_a_token_do_not_count() {
        assert_eq!(formatter_violations("'a\r\nb'\r\n", "'a\nb'\n"), []);
    }

    #[test]
    fn commas_that_are_tokens_do_not_count() {
        assert_eq!(formatter_violations("[a, b,]\n", "[a b]\n"), []);
        assert_eq!(
            formatter_violations("[a\n, # c\nb]\n", "[\n    a\n    # c\n    b\n]\n"),
            []
        );
        for (src, output) in [("echo , a\n", "echo a\n"), ("echo a,b\n", "echo ab\n")] {
            assert_eq!(
                invariant_names(&formatter_violations(src, output)),
                [Invariant::Tokens.name()],
                "{src:?}"
            );
        }
    }

    #[test]
    fn the_output_must_keep_every_comment_where_it_stands() {
        assert_eq!(
            invariant_names(&formatter_violations("ls # c\n", "ls\n")),
            [Invariant::Comments.name()]
        );
        let violations = formatter_violations("ls # c\n", "# c\nls\n");
        assert_eq!(invariant_names(&violations), [Invariant::Comments.name()]);
        assert_eq!(
            violations[0].detail,
            "comment 1 is \"# c\" at the end of a line in the source \
             and \"# c\" on its own line in the output"
        );
        let violations = formatter_violations("# c\n\nls\n", "# c\nls\n");
        assert_eq!(invariant_names(&violations), [Invariant::Comments.name()]);
        assert_eq!(
            violations[0].detail,
            "comment 1 is \"# c\" on its own line with a blank line after it in the source \
             and \"# c\" on its own line in the output"
        );
        let violations = formatter_violations("# c\nls\n", "# c\n\nls\n");
        assert_eq!(invariant_names(&violations), [Invariant::Comments.name()]);
        let violations = formatter_violations("ls\n", "ls # c\n");
        assert_eq!(invariant_names(&violations), [Invariant::Comments.name()]);
        assert_eq!(
            violations[0].detail,
            "comment 1, \"# c\" at the end of a line, is not in the source"
        );
    }

    #[test]
    fn trailing_whitespace_and_blank_lines_at_the_end_do_not_count() {
        assert_eq!(formatter_violations("ls # c  \n", "ls # c\n"), []);
        assert_eq!(formatter_violations("# c\n\n\n", "# c\n"), []);
        assert_eq!(formatter_violations("# a\n\n\n# b\n", "# a\n\n# b\n"), []);
        assert_eq!(
            formatter_violations(
                "echo {\nls\n# c\n\n}\nls\n",
                "echo {\n    ls\n    # c\n}\nls\n"
            ),
            []
        );
    }

    #[test]
    fn blank_lines_between_statements_of_a_block_count() {
        let violations =
            formatter_violations("echo {\n# c\n\nls\n}\n", "echo {\n    # c\n    ls\n}\n");
        assert_eq!(invariant_names(&violations), [Invariant::Comments.name()]);
    }

    #[test]
    fn blank_lines_between_arms_count() {
        let violations = formatter_violations(
            "match $x {\n# c\n\n1 => a\n}\n",
            "match $x {\n    # c\n    1 => a\n}\n",
        );
        assert_eq!(invariant_names(&violations), [Invariant::Comments.name()]);
    }

    #[test]
    fn blank_lines_inside_a_statement_do_not_count() {
        assert_eq!(
            formatter_violations("ls |\n# c\n\n| first\n", "ls |\n# c\n| first\n"),
            []
        );
    }

    #[test]
    fn the_output_must_not_change_when_formatted_again() {
        let violations = formatter_violations("ls|first\n", "ls |  first\n");
        assert_eq!(
            invariant_names(&violations),
            [Invariant::Idempotency.name()]
        );
        assert_eq!(
            violations[0].detail,
            "line 1 of the output, \"ls |  first\", becomes \"ls | first\" when formatted again"
        );
        let violations = formatter_violations("ls\nls\n", "ls\n\n\nls\n");
        assert_eq!(
            invariant_names(&violations),
            [Invariant::Idempotency.name()]
        );
        assert_eq!(
            violations[0].detail,
            "line 3 of the output, \"\", becomes \"ls\" when formatted again"
        );
        let violations = formatter_violations("ls\n", "ls\n\n");
        assert_eq!(
            invariant_names(&violations),
            [Invariant::Idempotency.name()]
        );
        assert_eq!(
            violations[0].detail,
            "line 2 of the output, \"\", is dropped when formatted again"
        );
    }

    #[test]
    fn a_change_of_line_ending_is_reported_by_byte() {
        let violations = formatter_violations("ls\n", "ls\r\n");
        assert_eq!(
            invariant_names(&violations),
            [Invariant::Idempotency.name()]
        );
        assert_eq!(
            violations[0].detail,
            "the output differs from byte 2 when formatted again"
        );
    }

    #[test]
    fn the_output_must_parse() {
        let violations = formatter_violations("ls\n", "ls |");
        assert_eq!(
            invariant_names(&violations),
            [Invariant::Idempotency.name()]
        );
        assert_eq!(
            violations[0].detail,
            "the output does not parse: no command after `|` at \"|\""
        );
    }

    #[test]
    fn a_violation_prints_in_one_line() {
        let violation = &formatter_violations("ls\n", "ls # c\n")[0];
        assert_eq!(
            violation.to_string(),
            "comment preservation: comment 1, \"# c\" at the end of a line, is not in the source"
        );
    }
}
