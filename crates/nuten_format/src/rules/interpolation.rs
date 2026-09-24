use nuten_doc::{Element, flat};
use nuten_syntax::{
    NodeOrToken, SyntaxKind,
    ast::{self, AstNode, Interpolation, Node, Subexpression},
};

use super::Formatter;

impl<'a> Formatter<'a> {
    pub(super) fn interpolation(&mut self, interpolation: &Interpolation) -> Vec<Element<'a>> {
        interpolation
            .syntax()
            .children_with_tokens()
            .flat_map(|piece| match ast::typed(piece) {
                NodeOrToken::Node(Node::Subexpression(subexpression)) => {
                    self.embedded(&subexpression)
                },
                piece => self.element(&piece),
            })
            .collect()
    }

    /// A subexpression inside a string, where a line break also splits the string across lines.
    /// So the layout adds one only where the source has one.
    /// One that cannot be flat stays as written, rather than breaking where the source does not.
    fn embedded(&mut self, subexpression: &Subexpression) -> Vec<Element<'a>> {
        let syntax = subexpression.syntax();
        let elements = self.subexpression(subexpression);
        if syntax
            .descendants_with_tokens()
            .any(|element| element.kind() == SyntaxKind::Newline)
        {
            return elements;
        }
        flat(&elements).unwrap_or_else(|| vec![self.source(syntax.text_range())])
    }
}
