use nuten_doc::{Build, expand_parent, soft_line_break};
use nuten_syntax::ast::{Bracketed, Subexpression};

use super::{Child, Formatter, children};

impl Formatter<'_> {
    pub(super) fn subexpression(&mut self, subexpression: &Subexpression) {
        let open = self.token(&subexpression.open());
        let close = self.token(&subexpression.close());
        let statements: Vec<_> = children(subexpression.inside(), &self.kept).collect();
        let Some(first) = statements.first() else {
            self.extend([open, close]);
            return;
        };
        // A comment last would swallow the `)`, and one at the end of a line would reach past it.
        let expand = statements.iter().any(Child::is_comment) || first.newlines > 0;
        // A break before a comment that stays on the `(` line would indent the space it brings.
        let leading_break = !first.is_comment() || first.newlines > 0;
        self.push(open);
        self.group(|this| {
            this.indent(|this| {
                if leading_break {
                    this.push(soft_line_break());
                }
                this.statements(&statements, false);
            });
            this.extend([soft_line_break(), close]);
            if expand {
                this.push(expand_parent());
            }
        });
    }
}
