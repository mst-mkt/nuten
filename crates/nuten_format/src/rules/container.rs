use std::iter;

use nuten_doc::{
    Element, empty_line, expand_parent, group, indent, soft_line_break, soft_line_break_or_space,
};
use nuten_syntax::{
    NodeOrToken, SyntaxElement, SyntaxKind, SyntaxNode, SyntaxToken,
    ast::{AstNode, Bracketed, RecordField, Table},
};

use super::{Child, Formatter, children, without_commas};

/// Makes a line break that prints as nothing or as a space when the group is flat.
type SoftLine = fn() -> Element<'static>;

impl<'a> Formatter<'a> {
    /// A list, a record, the parameters of a `def`, or the arms of a `match`, unless it stays as it is: its items after the opening bracket, with no space inside the brackets of a list or a signature and one inside those of a record.
    /// The arms always go one per line.
    pub(super) fn container(&mut self, container: &dyn Bracketed) -> Vec<Element<'a>> {
        if stays_as_is(container.syntax()) {
            return vec![self.source(container.syntax().text_range())];
        }
        let kind = container.syntax().kind();
        let line = if kind == SyntaxKind::Record {
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
        )
    }

    /// A table, unless it stays as it is: its rows as the items of a list, after its column names and, glued to them, their `;`, which stay on the line of the opening bracket, one space before the rows on one line.
    /// A line break right after the `[` expands it as one after the `;` does.
    pub(super) fn table(&mut self, table: &Table) -> Vec<Element<'a>> {
        if stays_as_is(table.syntax()) {
            return vec![self.source(table.syntax().text_range())];
        }
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
        )
    }

    /// `items` after `head`, which stays on the line of the opening bracket, then the closing bracket `close`.
    /// On one line when it fits, `expand` is false, no line break comes before the first item, and no comment is inside, with `before_first` before the first item and `before_close` before the closing bracket; otherwise one item per line, indented, with the closing bracket on a line of its own.
    /// A `,` is dropped.
    /// A blank line between items stays.
    fn items(
        &mut self,
        head: Vec<Element<'a>>,
        items: impl Iterator<Item = Child>,
        expand: bool,
        before_first: SoftLine,
        before_close: SoftLine,
        close: &SyntaxToken,
    ) -> Vec<Element<'a>> {
        let items: Vec<_> = without_commas(items).collect();
        let expand = expand
            || items.first().is_some_and(|first| first.newlines > 0)
            || items.iter().any(Child::is_comment);
        let inner: Vec<_> = items
            .iter()
            .enumerate()
            .flat_map(|(index, item)| self.item(index, item, before_first))
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

    /// The item at `index` after what separates it from the one before: `before_first` for the first, a blank line where the source had one, or else a line break or a space.
    /// A comment brings its own line break, so only a blank line comes before it.
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

    /// A key, its `:` right after it, and its value one space after that.
    /// What follows a comment inside starts a line.
    /// A `,` is dropped.
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
        self.spaced_children(&items, |index, _| Some(index) == colon)
    }
}

/// Whether `container` stays as it is: a comment inside ending in a lone `\r` keeps it so, since the line break printed after the comment would make that `\r` part of the line break.
fn stays_as_is(container: &SyntaxNode) -> bool {
    container
        .children_with_tokens()
        .flat_map(|child| {
            let field = child
                .as_node()
                .filter(|node| node.kind() == SyntaxKind::RecordField)
                .map(SyntaxNode::children_with_tokens);
            let own = field.is_none().then_some(child);
            own.into_iter().chain(field.into_iter().flatten())
        })
        .filter_map(SyntaxElement::into_token)
        .any(|token| token.kind() == SyntaxKind::Comment && token.text().ends_with('\r'))
}
