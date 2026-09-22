use nuten_doc::{Element, empty_line, expand_parent, group, indent, soft_line_break_or_space};
use nuten_syntax::{
    NodeOrToken, SyntaxKind,
    ast::{AstNode, Block, Bracketed},
};

use super::{Child, Formatter, children};

/// A block laid out for its container to group.
pub(super) struct BlockLayout<'a> {
    /// The opening brace and the parameters, which stay out of the group.
    /// Parameters over lines then leave the rest to fit after their last line.
    pub(super) head: Vec<Element<'a>>,
    pub(super) rest: Vec<Element<'a>>,
    /// Whether a line break comes after the parameters in the source.
    line_break: bool,
    /// Whether it holds at most one statement and nothing else.
    pub(super) one_statement: bool,
}

impl<'a> Formatter<'a> {
    pub(super) fn block(&mut self, block: &Block) -> Vec<Element<'a>> {
        let BlockLayout {
            mut head,
            mut rest,
            line_break,
            ..
        } = self.block_layout(block);
        if line_break {
            rest.push(expand_parent());
        }
        head.push(group(rest));
        head
    }

    pub(super) fn block_layout(&mut self, block: &Block) -> BlockLayout<'a> {
        let mut head = vec![self.token(&block.open())];
        let params = block.params();
        if let Some(params) = &params {
            head.push(self.source(params.syntax().text_range()));
        }
        let close = self.token(&block.close());
        let body: Vec<_> = children(block.statements()).collect();
        let one_statement = matches!(
            body.as_slice(),
            [] | [Child {
                element: NodeOrToken::Node(_),
                ..
            }]
        );
        let Some(first) = body.first() else {
            return BlockLayout {
                head,
                rest: vec![close],
                line_break: false,
                one_statement,
            };
        };
        let line_break = block
            .statements()
            .any(|element| element.kind() == SyntaxKind::Newline);
        let lead = if first.is_comment() && first.newlines == 0 {
            None
        } else if params.is_none()
            && first
                .element
                .as_node()
                .and_then(|statement| statement.syntax().first_child_or_token())
                .is_some_and(|first| first.kind() == SyntaxKind::Pipe)
        {
            // Right after the brace and a line break, nu would read the `|` as parameters.
            Some(empty_line())
        } else {
            Some(soft_line_break_or_space())
        };
        let inner = lead.into_iter().chain(self.statements(&body, false));
        BlockLayout {
            head,
            rest: vec![indent(inner), soft_line_break_or_space(), close],
            line_break,
            one_statement,
        }
    }
}
