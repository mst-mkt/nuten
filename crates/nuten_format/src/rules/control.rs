use nuten_doc::{Build, expand_parent};
use nuten_syntax::{
    NodeOrToken, SyntaxKind, SyntaxNode,
    ast::{self, Block, Node},
};

use super::{Child, Formatter, block::holds_at_most_one_statement, children, line_break_or_space};

impl Formatter<'_> {
    pub(super) fn bodies(&mut self, keyword: &SyntaxNode) {
        let items: Vec<_> = children(keyword.children_with_tokens(), &self.kept).collect();
        let expand = !matches!(keyword.kind(), SyntaxKind::If | SyntaxKind::Try)
            || is_statement(keyword)
            || (0..items.len())
                .filter_map(|index| body_at(&items, index))
                .any(|body| !holds_at_most_one_statement(body, &self.kept));
        self.group(|this| {
            this.spaced_children_by(
                &items,
                |_, item| Some(line_break_or_space(item)),
                |this, index, element| match body_at(&items, index) {
                    Some(body) if !this.kept.keeps(&ast::untyped(element)) => {
                        this.ungrouped_block(body);
                    },
                    _ => this.element(element),
                },
            );
            if expand {
                this.push(expand_parent());
            }
        });
    }
}

fn body_at(items: &[Child], index: usize) -> Option<&Block> {
    let NodeOrToken::Node(Node::Block(block)) = &items[index].element else {
        return None;
    };
    items[index + 1..]
        .iter()
        .find(|next| !next.is_comment())
        .is_none_or(|next| {
            next.element.as_token().is_some_and(|next| {
                next.kind() == SyntaxKind::Word && matches!(next.text(), "else" | "catch")
            })
        })
        .then_some(block)
}

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
            .take(2)
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
