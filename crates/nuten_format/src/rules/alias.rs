use nuten_doc::space;
use nuten_syntax::{
    NodeOrToken,
    ast::{self, Alias, AstNode, Node},
};

use super::{Formatter, children};

impl Formatter<'_> {
    /// Nu reads what the alias stands for as one call, where a `|` is an argument.
    /// So a line break there ends no element, unlike in a pipeline.
    pub(super) fn alias(&mut self, alias: &Alias) {
        let items: Vec<_> = children(alias.syntax().children_with_tokens(), &self.kept)
            .flat_map(|child| match &child.element {
                NodeOrToken::Node(call @ (Node::Call(_) | Node::ExternalCall(_)))
                    if !self.kept.keeps(&ast::untyped(&child.element)) =>
                {
                    children(call.syntax().children_with_tokens(), &self.kept).collect()
                },
                _ => vec![child],
            })
            .collect();
        self.spaced_children(&items, |_, _| Some(space()));
    }
}
