use nuten_doc::Build;
use nuten_syntax::{
    NodeOrToken, SyntaxKind,
    ast::{self, AstNode, Interpolation, Node, Subexpression},
};

use super::Formatter;

impl Formatter<'_> {
    pub(super) fn interpolation(&mut self, interpolation: &Interpolation) {
        for piece in interpolation.syntax().children_with_tokens() {
            match (self.kept.keeps(&piece), ast::typed(piece)) {
                (false, NodeOrToken::Node(Node::Subexpression(subexpression))) => {
                    self.embedded(&subexpression);
                },
                (_, piece) => self.element(&piece),
            }
        }
    }

    /// A line break here also splits the string, so only the source may place one.
    fn embedded(&mut self, subexpression: &Subexpression) {
        let syntax = subexpression.syntax();
        if syntax
            .descendants_with_tokens()
            .any(|element| element.kind() == SyntaxKind::Newline)
        {
            self.subexpression(subexpression);
        } else {
            self.flat_or(
                |this| this.subexpression(subexpression),
                |this| this.source(syntax.text_range()),
            );
        }
    }
}
