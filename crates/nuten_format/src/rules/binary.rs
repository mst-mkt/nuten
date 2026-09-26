use nuten_doc::{Element, empty_line, hard_line_break, indent, space};
use nuten_syntax::{
    SyntaxKind,
    ast::{AstNode, Binary, Bracketed},
};

use super::{Child, Formatter, children, without_commas};

impl<'a> Formatter<'a> {
    /// The items stay on the lines they are on, since the author may group the bytes by line.
    /// Nu joins the items before it decodes them.
    /// So the breaks and spaces among them change nothing.
    pub(super) fn binary(&mut self, binary: &Binary) -> Vec<Element<'a>> {
        let items: Vec<_> = without_commas(children(binary.inside(), &self.kept)).collect();
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
            return self.source(binary.syntax().text_range());
        }
        let inner = self.spaced_children(&items, |_, item| {
            Some(match item.newlines {
                0 => space(),
                1 => hard_line_break(),
                _ => empty_line(),
            })
        });
        let body = if items.is_empty() || !multiline {
            inner
        } else {
            // A break before a comment on the `[` line would move it to a line of its own.
            let leading_break = !items[0].is_comment() || items[0].newlines > 0;
            let lines = leading_break.then(hard_line_break).into_iter().chain(inner);
            vec![indent(lines), hard_line_break()]
        };
        [self.token(&binary.prefix()), self.token(&binary.open())]
            .into_iter()
            .chain(body)
            .chain([self.token(&binary.close())])
            .collect()
    }
}
