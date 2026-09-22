use nuten_doc::{Element, expand_parent, group};
use nuten_syntax::{NodeOrToken, SyntaxKind, SyntaxNode, ast::Node};

use super::{Child, Formatter, children, line_break_or_space, separator};

impl<'a> Formatter<'a> {
    pub(super) fn bodies(&mut self, keyword: &SyntaxNode) -> Vec<Element<'a>> {
        let items: Vec<_> = children(keyword.children_with_tokens()).collect();
        let mut expand =
            !matches!(keyword.kind(), SyntaxKind::If | SyntaxKind::Try) || is_statement(keyword);
        let mut out = Vec::new();
        for (index, item) in items.iter().enumerate() {
            if let Some(comment) = item.as_comment() {
                out.extend(self.comment(comment, item.newlines, false));
                continue;
            }
            let prev = items[..index].last().map(Child::kind);
            out.extend(separator(prev, Some(line_break_or_space(item))));
            // A body comes right before an `else` or a `catch`.
            // The last item but a comment is a body, or what the last `else` takes.
            let branch = items[index + 1..]
                .iter()
                .find(|next| !next.is_comment())
                .is_none_or(|next| {
                    next.element.as_token().is_some_and(|next| {
                        next.kind() == SyntaxKind::Word && matches!(next.text(), "else" | "catch")
                    })
                });
            match &item.element {
                NodeOrToken::Node(Node::Block(block)) if branch => {
                    let layout = self.block_layout(block);
                    expand |= !layout.one_statement;
                    out.extend(layout.head);
                    out.extend(layout.rest);
                },
                element => out.extend(self.element(element)),
            }
        }
        if expand {
            out.push(expand_parent());
        }
        vec![group(out)]
    }
}

/// Whether `node`, an `If` or a `Try`, is a statement.
/// It must be the only element of its pipeline, redirections aside.
/// That pipeline must be a statement of Root, or of a block.
/// A block counts only when it holds more than that pipeline, its braces, and its parameters.
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
