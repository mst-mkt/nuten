use nuten_doc::{Element, expand_parent, group, indent, soft_line_break};
use nuten_syntax::{
    SyntaxKind,
    ast::{Bracketed, Subexpression},
};

use super::{Child, Formatter, children};

impl<'a> Formatter<'a> {
    pub(super) fn subexpression(&mut self, subexpression: &Subexpression) -> Vec<Element<'a>> {
        let open = self.token(&subexpression.open());
        let close = self.token(&subexpression.close());
        let statements: Vec<_> = children(subexpression.inside()).collect();
        if statements.is_empty() {
            return vec![open, close];
        }
        // A comment last would swallow the `)`, and one at the end of a line would reach past it.
        let expand = statements.iter().any(Child::is_comment)
            || subexpression
                .inside()
                .any(|element| element.kind() == SyntaxKind::Newline);
        // A break before a comment that stays on the `(` line would indent the space it brings.
        let leading_break = !statements[0].is_comment() || statements[0].newlines > 0;
        let inner = leading_break
            .then(soft_line_break)
            .into_iter()
            .chain(self.statements(&statements, false));
        let body = [indent(inner), soft_line_break(), close];
        vec![
            open,
            group(body.into_iter().chain(expand.then(expand_parent))),
        ]
    }
}
