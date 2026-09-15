//! What the parser must keep, asked of any input, with syntax errors or
//! without.

use std::fmt;

use nuten_syntax::{SyntaxKind, SyntaxNode};

use crate::{ParseError, Parsed};

/// One property of the parser.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Invariant {
    /// The tree restores the source byte for byte.
    Roundtrip,
    /// Every node but Root holds text, and its first and last tokens are not
    /// trivia.
    NodeBounds,
    /// The errors match the `Error` elements one to one, in order, each
    /// inside its element.
    ErrorPlacement,
}

impl Invariant {
    /// The name in reports.
    pub fn name(self) -> &'static str {
        match self {
            Invariant::Roundtrip => "roundtrip",
            Invariant::NodeBounds => "node bounds",
            Invariant::ErrorPlacement => "error placement",
        }
    }
}

/// An invariant that does not hold for one input.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Violation {
    pub invariant: Invariant,
    /// What breaks first, in one line.
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
/// `parsed` is the tree of `src`.
pub fn violations(src: &str, parsed: &Parsed) -> Vec<Violation> {
    let tree = parsed.syntax();
    [
        roundtrip(src, &tree),
        node_bounds(&tree),
        error_placement(&tree, parsed.errors()),
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
    Some(Violation::new(
        Invariant::Roundtrip,
        format!("the tree differs from the source from byte {offset}"),
    ))
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
        Some(Violation::new(
            Invariant::NodeBounds,
            format!("{:?} at {:?} {flaw}", node.kind(), node.text_range()),
        ))
    })
}

fn error_placement(tree: &SyntaxNode, errors: &[ParseError]) -> Option<Violation> {
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
    Some(Violation::new(Invariant::ErrorPlacement, detail))
}

#[cfg(test)]
mod tests {
    use nuten_syntax::SyntaxKind::{
        self, Call, Error, Newline, Pipe, Pipeline, Root, Whitespace, Word,
    };
    use rowan::{GreenNodeBuilder, TextRange};

    use super::{Invariant, Violation, violations};
    use crate::{ParseError, ParseErrorKind, Parsed, parse, test_support::short_inputs};

    /// Root with one node of `kind` that holds `tokens`.
    fn one_node(
        kind: SyntaxKind,
        tokens: &[(SyntaxKind, &str)],
        errors: Vec<ParseError>,
    ) -> Parsed {
        let mut builder = GreenNodeBuilder::new();
        builder.start_node(Root.to_raw());
        builder.start_node(kind.to_raw());
        for &(token, text) in tokens {
            builder.token(token.to_raw(), text);
        }
        builder.finish_node();
        builder.finish_node();
        Parsed {
            green: builder.finish(),
            errors,
        }
    }

    fn violation(invariant: Invariant, detail: &str) -> Violation {
        Violation::new(invariant, detail.to_string())
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
    fn errors_must_match_error_elements() {
        let parsed = one_node(Error, &[(Pipe, "|")], vec![]);
        assert_eq!(
            violations("|", &parsed),
            [violation(
                Invariant::ErrorPlacement,
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
                Invariant::ErrorPlacement,
                "error 1 at 2..3 is outside its Error element at 0..2"
            )]
        );
    }

    #[test]
    fn a_violation_prints_in_one_line() {
        assert_eq!(
            violations("ls | first", &parse("ls | get"))[0].to_string(),
            "roundtrip: the tree differs from the source from byte 5"
        );
    }
}
