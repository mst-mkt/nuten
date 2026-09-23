use nuten_doc::Element;
use nuten_syntax::ast::{AstNode, Module};

use super::{Formatter, children, line_break_or_space};

impl<'a> Formatter<'a> {
    pub(super) fn module(&mut self, module: &Module) -> Vec<Element<'a>> {
        let items: Vec<_> = children(module.syntax().children_with_tokens()).collect();
        let body = module.body();
        self.spaced_children_by(
            &items,
            |_, item| Some(line_break_or_space(item)),
            |this, element| this.element_or_body(element, body.as_ref()),
        )
    }
}
