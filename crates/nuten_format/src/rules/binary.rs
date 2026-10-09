use nuten_doc::{Build, empty_line, hard_line_break, space};
use nuten_syntax::{
    SyntaxKind,
    ast::{AstNode, Binary, Bracketed},
};

use super::{Child, Formatter, children, without_separators};

impl Formatter<'_> {
    /// The items stay on the lines they are on, since the author may group the bytes by line.
    /// Nu joins the items before it decodes them.
    /// So the breaks and spaces among them change nothing.
    pub(super) fn binary(&mut self, binary: &Binary) {
        let items: Vec<_> = without_separators(children(binary.inside(), &self.kept)).collect();
        // Nu ends a comment in the word at a lone `\r`, but one among the items at the line break.
        // What comes between is in the comment alone, so only the source can place it.
        let cr_inside_comment = items
            .iter()
            .filter_map(Child::as_comment)
            .any(|comment| comment.text().trim_end_matches('\r').contains('\r'));
        let multiline = binary
            .inside()
            .any(|element| element.kind() == SyntaxKind::Newline);
        // On one line with the items, a comment ends at a lone `\r` before the `]`.
        // As a line suffix it would move past the `]`.
        // Breaking before the `]` instead would put items on lines they are not on.
        let one_line_with_comment = !multiline && items.iter().any(Child::is_comment);
        if cr_inside_comment || one_line_with_comment {
            self.source(binary.syntax().text_range());
            return;
        }
        self.extend([self.token(&binary.prefix()), self.token(&binary.open())]);
        let inner = |this: &mut Self| {
            this.spaced_children(&items, |_, item| {
                Some(match item.newlines {
                    0 => space(),
                    1 => hard_line_break(),
                    _ => empty_line(),
                })
            });
        };
        if items.is_empty() || !multiline {
            inner(self);
        } else {
            // A break before a comment on the `[` line would move it to a line of its own.
            let leading_break = !items[0].is_comment() || items[0].newlines > 0;
            self.indent(|this| {
                if leading_break {
                    this.push(hard_line_break());
                }
                inner(this);
            });
            self.push(hard_line_break());
        }
        self.push(self.token(&binary.close()));
    }
}
