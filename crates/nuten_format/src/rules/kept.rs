//! What `# nuten-ignore` keeps as written.

use std::{cmp::Reverse, collections::HashMap, iter};

use nuten_syntax::{
    NodeOrToken, SyntaxElement, SyntaxKind, SyntaxNode, SyntaxToken, TextRange, TextSize,
};

const IGNORE: &str = "nuten-ignore";

/// The runs of children kept as written.
/// A run is one child, or siblings from a first to a last with all between them.
/// The first child of a run stands for all of it, so `children` drops the rest.
#[derive(Debug, Default)]
pub(super) struct Kept {
    members: HashMap<SyntaxElement, Member>,
}

#[derive(Debug, Clone, Copy)]
pub(super) enum Member {
    /// The first child, with the source of the whole run.
    First(TextRange),
    /// A child after the first, printed by the first.
    Rest,
}

impl Kept {
    /// `root` is the tree of `src`.
    pub(super) fn of(src: &str, root: &SyntaxNode) -> Kept {
        // Most files hold no such comment, and the text tells that sooner than the tree.
        if !src.contains(IGNORE) {
            return Kept::default();
        }
        let runs = root
            .descendants_with_tokens()
            .filter_map(SyntaxElement::into_token)
            .filter(|token| token.kind() == SyntaxKind::Comment && ignores(token.text()))
            .filter_map(|comment| {
                if stands_on_own_line(&comment) {
                    run_after(&comment)
                } else {
                    run_before(&comment)
                }
            })
            .collect();
        let members = outermost(runs)
            .into_iter()
            .flat_map(|run| {
                let members =
                    iter::once(Member::First(span(&run))).chain(iter::repeat(Member::Rest));
                run.into_iter().zip(members)
            })
            .collect();
        Kept {
            members,
        }
    }

    pub(super) fn member(&self, element: &SyntaxElement) -> Option<Member> {
        self.members.get(element).copied()
    }

    pub(super) fn is_rest(&self, element: &SyntaxElement) -> bool {
        matches!(self.member(element), Some(Member::Rest))
    }

    /// Whether `element` is in a run, first or not.
    pub(super) fn keeps(&self, element: &SyntaxElement) -> bool {
        self.member(element).is_some()
    }
}

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
fn run_after(comment: &SyntaxToken) -> Option<Vec<SyntaxElement>> {
    iter::successors(
        comment.next_sibling_or_token(),
        SyntaxElement::next_sibling_or_token,
    )
    .find(|sibling| !sibling.kind().is_trivia() && sibling.kind() != SyntaxKind::Pipe)
    .filter(is_keepable)
    .map(|sibling| vec![sibling])
}

/// The run that `comment` ends the line of.
/// The line decides rather than where the comment is, since a comment after a `{` is inside it.
/// A `,` before the comment is passed over, since the layout drops it or puts one there.
/// A bracket opened on an earlier line ends no run.
/// The layout may move that bracket to a line of its own, and the comment with it.
fn run_before(comment: &SyntaxToken) -> Option<Vec<SyntaxElement>> {
    let before: Vec<_> = iter::successors(comment.prev_token(), SyntaxToken::prev_token)
        .take_while(|token| token.kind() != SyntaxKind::Newline)
        .collect();
    let line_start = before.last()?.text_range().start();
    let last = before
        .iter()
        .find(|token| !matches!(token.kind(), SyntaxKind::Whitespace | SyntaxKind::Comma))
        .map(|token| outermost_on_line(token.clone(), line_start))
        .filter(is_keepable)?;
    let first = before
        .into_iter()
        .rev()
        .map(|token| outermost_on_line(token, line_start))
        .find(|top| is_keepable(top) && top.parent() == last.parent())?;
    Some(siblings_between(&first, &last))
}

/// The outermost element that holds `token` and starts on the line from `line_start`.
/// Never the file itself, which starts on the first line but is no child to keep.
fn outermost_on_line(token: SyntaxToken, line_start: TextSize) -> SyntaxElement {
    iter::successors(Some(SyntaxElement::from(token)), |element| {
        element
            .parent()
            .filter(|parent| {
                parent.kind() != SyntaxKind::Root && parent.text_range().start() >= line_start
            })
            .map(SyntaxElement::from)
    })
    .last()
    .expect("`successors` yields its first element")
}

/// A node or an item, since the layout of anything else is its container's.
/// The parameters of a closure are no child to keep either, since its block puts them by its `{`.
fn is_keepable(element: &SyntaxElement) -> bool {
    match element {
        NodeOrToken::Node(node) => node.kind() != SyntaxKind::ClosureParams,
        NodeOrToken::Token(token) => token.kind().is_item(),
    }
}

fn siblings_between(first: &SyntaxElement, last: &SyntaxElement) -> Vec<SyntaxElement> {
    let end = last.text_range().start();
    iter::successors(Some(first.clone()), SyntaxElement::next_sibling_or_token)
        .take_while(|sibling| sibling.text_range().start() <= end)
        .collect()
}

/// `runs` without those inside another.
/// Two runs never overlap otherwise, since each is of siblings that start on one line.
fn outermost(mut runs: Vec<Vec<SyntaxElement>>) -> Vec<Vec<SyntaxElement>> {
    runs.sort_by_key(|run| {
        let span = span(run);
        (span.start(), Reverse(span.end()))
    });
    runs.into_iter()
        .scan(TextSize::from(0), |end, run| {
            let span = span(&run);
            let outer = span.start() >= *end;
            if outer {
                *end = span.end();
            }
            Some(outer.then_some(run))
        })
        .flatten()
        .collect()
}

fn span(run: &[SyntaxElement]) -> TextRange {
    let first = run.first().expect("a run holds a child");
    let last = run.last().expect("a run holds a child");
    TextRange::new(first.text_range().start(), last.text_range().end())
}

#[cfg(test)]
mod tests {
    use super::ignores;

    #[test]
    fn ignore_comment_allows_spaces_and_tabs_before_name() {
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
    fn ignore_comment_allows_whitespace_and_text_after_name() {
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
