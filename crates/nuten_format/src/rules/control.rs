use nuten_doc::{Element, expand_parent, group, space};
use nuten_syntax::{NodeOrToken, SyntaxKind, SyntaxNode, ast::Node};

use super::{Formatter, children};

impl<'a> Formatter<'a> {
    /// An `if` with its `else` branches, or a `for`, a `while`, a `loop`, or
    /// a `try`, its parts one space apart and its bodies laid out as blocks
    /// that break together: always but for an `if` or a `try` that is no
    /// statement, which stays on one line when every body holds one
    /// statement at most and the whole fits. Line breaks in the source do not
    /// count.
    pub(super) fn bodies(&mut self, keyword: &SyntaxNode) -> Vec<Element<'a>> {
        let items: Vec<_> = children(keyword.children_with_tokens())
            .map(|child| child.element)
            .collect();
        let mut expand =
            !matches!(keyword.kind(), SyntaxKind::If | SyntaxKind::Try) || is_statement(keyword);
        let mut out = Vec::new();
        for (index, item) in items.iter().enumerate() {
            if index > 0 {
                out.push(space());
            }
            // A body comes right before an `else` or a `catch`, and the last
            // item is a body or what the last `else` takes.
            let branch = items.get(index + 1).is_none_or(|next| {
                next.as_token().is_some_and(|next| {
                    next.kind() == SyntaxKind::Word && matches!(next.text(), "else" | "catch")
                })
            });
            match item {
                NodeOrToken::Node(Node::Block(block)) if branch => {
                    let layout = self.block_layout(block);
                    expand |= !layout.one_statement;
                    out.extend(layout.head);
                    out.extend(layout.rest);
                },
                _ => out.extend(self.element(item)),
            }
        }
        if expand {
            out.push(expand_parent());
        }
        vec![group(out)]
    }
}

/// Whether `node`, an `If` or a `Try`, is a statement: the only element of a
/// pipeline, redirections aside, that is a statement of Root, or of a block
/// holding anything else but its braces, parameters, and comments.
fn is_statement(node: &SyntaxNode) -> bool {
    let Some(pipeline) = node.parent().and_then(|element| element.parent()) else {
        return false;
    };
    let alone = |parent: &SyntaxNode| {
        parent
            .children_with_tokens()
            .filter(|child| {
                !child.kind().is_trivia()
                    && !matches!(
                        child.kind(),
                        SyntaxKind::LBrace | SyntaxKind::RBrace | SyntaxKind::ClosureParams
                    )
            })
            .count()
            == 1
    };
    pipeline.kind() == SyntaxKind::Pipeline
        && alone(&pipeline)
        && match pipeline.parent() {
            Some(parent) if parent.kind() == SyntaxKind::Root => true,
            Some(parent) if parent.kind() == SyntaxKind::Block => !alone(&parent),
            _ => false,
        }
}
