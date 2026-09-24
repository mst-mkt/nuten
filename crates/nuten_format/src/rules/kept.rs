//! What `# nuten-ignore` keeps as written.

use std::{collections::HashSet, iter};

use nuten_syntax::{NodeOrToken, SyntaxElement, SyntaxKind, SyntaxNode, SyntaxToken};

const IGNORE: &str = "nuten-ignore";

/// The children kept as written.
#[derive(Debug, Default)]
pub(super) struct Kept {
    children: HashSet<SyntaxElement>,
}

impl Kept {
    /// `root` is the tree of `src`.
    pub(super) fn of(src: &str, root: &SyntaxNode) -> Kept {
        // Most files hold no such comment, and the text tells that sooner than the tree.
        if !src.contains(IGNORE) {
            return Kept::default();
        }
        let children = root
            .descendants_with_tokens()
            .filter_map(SyntaxElement::into_token)
            .filter(|token| {
                token.kind() == SyntaxKind::Comment
                    && ignores(token.text())
                    && stands_on_own_line(token)
            })
            .filter_map(|comment| kept_after(&comment))
            .collect();
        Kept {
            children,
        }
    }

    pub(super) fn keeps(&self, element: &SyntaxElement) -> bool {
        self.children.contains(element)
    }
}

/// Whether `comment` is `# nuten-ignore`, alone or followed by whitespace and any text.
/// Any whitespace counts after the name, since a comment in a CRLF file ends with its `\r`.
fn ignores(comment: &str) -> bool {
    comment
        .strip_prefix('#')
        .and_then(|rest| rest.trim_start_matches([' ', '\t']).strip_prefix(IGNORE))
        .is_some_and(|rest| rest.is_empty() || rest.starts_with(char::is_whitespace))
}

fn stands_on_own_line(comment: &SyntaxToken) -> bool {
    iter::successors(comment.prev_token(), SyntaxToken::prev_token)
        .find(|token| token.kind() != SyntaxKind::Whitespace)
        .is_none_or(|token| token.kind() == SyntaxKind::Newline)
}

/// The sibling after `comment`, past comments, line breaks, and the `|` of a line-start `|`.
fn kept_after(comment: &SyntaxToken) -> Option<SyntaxElement> {
    iter::successors(
        comment.next_sibling_or_token(),
        SyntaxElement::next_sibling_or_token,
    )
    .find(|sibling| !sibling.kind().is_trivia() && sibling.kind() != SyntaxKind::Pipe)
    .filter(is_keepable)
}

/// A node or an item, since the layout of anything else is its container's.
fn is_keepable(element: &SyntaxElement) -> bool {
    match element {
        NodeOrToken::Node(_) => true,
        NodeOrToken::Token(token) => token.kind().is_item(),
    }
}

#[cfg(test)]
mod tests {
    use super::ignores;

    #[test]
    fn ignore_comment_takes_spaces_and_tabs_before_name() {
        for comment in [
            "# nuten-ignore",
            "#nuten-ignore",
            "#\tnuten-ignore",
            "#  nuten-ignore",
        ] {
            assert!(ignores(comment), "{comment:?}");
        }
    }

    #[test]
    fn ignore_comment_takes_whitespace_and_text_after_name() {
        for comment in [
            "# nuten-ignore laid out as a table",
            "# nuten-ignore\tlaid out as a table",
            "# nuten-ignore\r",
            "# nuten-ignore \r",
        ] {
            assert!(ignores(comment), "{comment:?}");
        }
    }

    #[test]
    fn other_comment_ignores_nothing() {
        for comment in [
            "# nuten-ignored",
            "# nuten-ignore: laid out as a table",
            "## nuten-ignore",
            "# see nuten-ignore",
            "# nuten",
        ] {
            assert!(!ignores(comment), "{comment:?}");
        }
    }
}
