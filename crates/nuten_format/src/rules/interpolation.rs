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
            .flat_map(|piece| match (self.kept.keeps(&piece), ast::typed(piece)) {
                (false, NodeOrToken::Node(Node::Subexpression(subexpression))) => {
                    self.embedded(&subexpression)
                },
                (_, piece) => self.element(&piece),
            })
            .collect()
    }

    /// A line break here also splits the string, so only the source may place one.
    fn embedded(&mut self, subexpression: &Subexpression) -> Vec<Element<'a>> {
        let syntax = subexpression.syntax();
        let elements = self.subexpression(subexpression);
        if syntax
            .descendants_with_tokens()
            .any(|element| element.kind() == SyntaxKind::Newline)
        {
            return elements;
        }
        flat(&self.document, &elements).unwrap_or_else(|| self.source(syntax.text_range()))
    }
}
