use std::iter;

use nuten_doc::{
    Element, empty_line, expand_parent, group, indent, soft_line_break, soft_line_break_or_space,
    space, text,
};
use nuten_syntax::{
    NodeOrToken, SyntaxKind, SyntaxToken,
    ast::{AstNode, Bracketed, RecordField, Table},
};

use super::{Child, Formatter, children, without_commas};

/// Makes a line break that prints as nothing or as a space when the group is flat.
type SoftLine = fn() -> Element<'static>;

impl<'a> Formatter<'a> {
    pub(super) fn container(&mut self, container: &dyn Bracketed) -> Vec<Element<'a>> {
        let kind = container.syntax().kind();
        let line = if matches!(kind, SyntaxKind::Record | SyntaxKind::RecordPattern) {
            soft_line_break_or_space
        } else {
            soft_line_break
        };
        let head = vec![self.token(&container.open())];
        self.items(
            head,
            children(container.inside()),
            kind == SyntaxKind::MatchArms,
            line,
            line,
            &container.close(),
            false,
        )
    }

    pub(super) fn table(&mut self, table: &Table) -> Vec<Element<'a>> {
        let expand = children(table.inside())
            .next()
            .is_some_and(|columns| columns.newlines > 0);
        let (columns, semicolon) = table.header();
        let mut head = vec![self.token(&table.open())];
        head.extend(self.element(&columns));
        head.push(self.token(&semicolon));
        self.items(
            head,
            children(table.following(&semicolon.into())),
            expand,
            soft_line_break_or_space,
            soft_line_break,
            &table.close(),
            false,
        )
    }

    /// `commas` puts a `,` after each item but the last, comments aside.
    #[expect(
        clippy::too_many_arguments,
        reason = "each container lays out its items with its own choice of these"
    )]
    pub(super) fn items(
        &mut self,
        head: Vec<Element<'a>>,
        items: impl Iterator<Item = Child>,
        expand: bool,
        before_first: SoftLine,
        before_close: SoftLine,
        close: &SyntaxToken,
        commas: bool,
    ) -> Vec<Element<'a>> {
        let items: Vec<_> = without_commas(items).collect();
        let expand = expand
            || items.first().is_some_and(|first| first.newlines > 0)
            || items.iter().any(Child::is_comment);
        let last = items.iter().rposition(|item| !item.is_comment());
        let inner: Vec<_> = items
            .iter()
            .enumerate()
            .flat_map(|(index, item)| {
                let comma = commas && !item.is_comment() && Some(index) != last;
                self.item(index, item, before_first)
                    .into_iter()
                    .chain(comma.then(|| text(",")))
            })
            .collect();
        let mut out = head;
        if inner.is_empty() {
            out.push(self.token(close));
            return out;
        }
        out.push(indent(inner));
        out.push(before_close());
        out.push(self.token(close));
        if expand {
            out.push(expand_parent());
        }
        vec![group(out)]
    }

    fn item(&mut self, index: usize, item: &Child, before_first: SoftLine) -> Vec<Element<'a>> {
        let Child {
            element,
            newlines,
        } = item;
        let blank_line = index > 0 && *newlines > 1;
        match element {
            NodeOrToken::Token(comment) if comment.kind() == SyntaxKind::Comment => blank_line
                .then(empty_line)
                .into_iter()
                .chain(self.comment(comment, *newlines, false))
                .collect(),
            _ => {
                let separator = if index == 0 {
                    before_first()
                } else if blank_line {
                    empty_line()
                } else {
                    // After a comment, the container is expanded, so the line break comes anyway.
                    soft_line_break_or_space()
                };
                iter::once(separator).chain(self.element(element)).collect()
            },
        }
    }

    pub(super) fn field(&mut self, field: &RecordField) -> Vec<Element<'a>> {
        let items: Vec<_> =
            without_commas(children(field.syntax().children_with_tokens())).collect();
        let colon = items
            .iter()
            .enumerate()
            .filter(|(_, item)| !item.is_comment())
            .nth(1)
            .filter(|(_, item)| item.kind() == SyntaxKind::Colon)
            .map(|(index, _)| index);
        self.spaced_children(&items, |index, _| (Some(index) != colon).then(space))
    }
}
