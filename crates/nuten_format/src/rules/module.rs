use nuten_doc::Element;
use nuten_syntax::{
    SyntaxNode,
    ast::{AstNode, Block, ExportEnv, Module},
};

use super::{Formatter, children, line_break_or_space};

impl<'a> Formatter<'a> {
    pub(super) fn module(&mut self, module: &Module) -> Vec<Element<'a>> {
        self.with_body_over_lines(module.syntax(), module.body().as_ref())
    }

    pub(super) fn export_env(&mut self, export_env: &ExportEnv) -> Vec<Element<'a>> {
        self.with_body_over_lines(export_env.syntax(), export_env.body().as_ref())
    }

    fn with_body_over_lines(
        &mut self,
        node: &SyntaxNode,
        body: Option<&Block>,
    ) -> Vec<Element<'a>> {
        let items: Vec<_> = children(node.children_with_tokens(), &self.kept).collect();
        self.spaced_children_by(
            &items,
            |_, item| Some(line_break_or_space(item)),
            |this, element| this.element_or_body(element, body),
        )
    }
}
