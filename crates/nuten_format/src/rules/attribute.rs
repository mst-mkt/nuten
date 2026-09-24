use nuten_doc::Element;
use nuten_syntax::{
    NodeOrToken,
    ast::{self, AstNode, Attribute, Node},
};

use super::{Formatter, children, line_break_or_space};

impl<'a> Formatter<'a> {
    /// Nu takes the text of a block in an attribute as the code of an example.
    /// `help` shows that text as it is written.
    pub(super) fn attribute(&mut self, attribute: &Attribute) -> Vec<Element<'a>> {
        let items: Vec<_> =
            children(attribute.syntax().children_with_tokens(), &self.kept).collect();
        self.spaced_children_by(
            &items,
            |_, item| Some(line_break_or_space(item)),
            |this, element| match element {
                NodeOrToken::Node(Node::Block(block))
                    if !this.kept.keeps(&ast::untyped(element)) =>
                {
                    vec![this.source(block.syntax().text_range())]
                },
                element => this.element(element),
            },
        )
    }
}
