use nuten_doc::{Element, expand_parent, group};
use nuten_syntax::{
    NodeOrToken, SyntaxKind, SyntaxNode,
    ast::{self, Block, Node},
};

use super::{Child, Formatter, block::holds_at_most_one_statement, children, line_break_or_space};

impl<'a> Formatter<'a> {
    pub(super) fn bodies(&mut self, keyword: &SyntaxNode) -> Vec<Element<'a>> {
        let items: Vec<_> = children(keyword.children_with_tokens(), &self.kept).collect();
        let bodies = body_blocks(&items);
        let expand = !matches!(keyword.kind(), SyntaxKind::If | SyntaxKind::Try)
            || is_statement(keyword)
            || bodies
                .iter()
                .any(|body| !holds_at_most_one_statement(body, &self.kept));
        let out = self.spaced_children_by(
            &items,
            |_, item| Some(line_break_or_space(item)),
            |this, element| match element {
                NodeOrToken::Node(Node::Block(block))
                    if bodies.contains(&block) && !this.kept.keeps(&ast::untyped(element)) =>
                {
                    let layout = this.block_layout(block);
                    layout.head.into_iter().chain(layout.rest).collect()
                },
                element => this.element(element),
            },
        );
        vec![group(out.into_iter().chain(expand.then(expand_parent)))]
    }
}

/// A body comes right before an `else` or a `catch`.
/// The last item but a comment is a body, or what the last `else` takes.
fn body_blocks(items: &[Child]) -> Vec<&Block> {
    items
        .iter()
        .enumerate()
        .filter(|(index, _)| {
            items[index + 1..]
                .iter()
                .find(|next| !next.is_comment())
                .is_none_or(|next| {
                    next.element.as_token().is_some_and(|next| {
                        next.kind() == SyntaxKind::Word && matches!(next.text(), "else" | "catch")
                    })
                })
        })
        .filter_map(|(_, item)| match &item.element {
            NodeOrToken::Node(Node::Block(block)) => Some(block),
            _ => None,
        })
        .collect()
}

/// Whether `node`, an `If` or a `Try`, is a statement.
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
