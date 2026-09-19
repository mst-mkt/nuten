use std::mem;

use nuten_doc::{
    Element, empty_line, expand_parent, group, hard_line_break, indent, soft_line_break,
    soft_line_break_or_space, space,
};
use nuten_syntax::{
    NodeOrToken, SyntaxElement, SyntaxKind, SyntaxNode, SyntaxToken,
    ast::{self, AstNode, Bracketed, Node, RecordField, Table},
};

use super::{Child, Formatter, children};

/// Makes a line break that prints as nothing or as a space when the group is
/// flat.
type SoftLine = fn() -> Element<'static>;

impl<'a> Formatter<'a> {
    /// A list, a record, or the parameters of a `def`, unless it stays as it
    /// is: its items after the opening bracket, with no space inside the
    /// brackets of a list or a signature and one inside those of a record.
    pub(super) fn container(&mut self, container: &dyn Bracketed) -> Vec<Element<'a>> {
        if stays_as_is(container.syntax()) {
            return vec![self.source(container.syntax().text_range())];
        }
        let line = if container.syntax().kind() == SyntaxKind::Record {
            soft_line_break_or_space
        } else {
            soft_line_break
        };
        let head = vec![self.token(&container.open())];
        self.items(
            head,
            children(container.inside()),
            false,
            line,
            line,
            &container.close(),
        )
    }

    /// A table, unless it stays as it is: its rows as the items of a list,
    /// after its column names and, glued to them, their `;`, which stay on the
    /// line of the opening bracket, one space before the rows on one line. A
    /// line break right after the `[` expands it as one after the `;` does.
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

    /// `items` after `head`, which stays on the line of the opening bracket,
    /// then the closing bracket `close`. On one line when it fits, `expand` is
    /// false, no line break comes before the first item, and no comment is
    /// inside, with `before_first` before the first item and `before_close`
    /// before the closing bracket; otherwise one item per line, indented, with
    /// the closing bracket on a line of its own. A `,` is dropped. A blank
    /// line between items stays.
    fn items(
        &mut self,
        head: Vec<Element<'a>>,
        items: impl Iterator<Item = Child>,
        expand: bool,
        before_first: SoftLine,
        before_close: SoftLine,
        close: &SyntaxToken,
    ) -> Vec<Element<'a>> {
        let mut items = items.peekable();
        let mut expand = expand || items.peek().is_some_and(|first| first.newlines > 0);
        let mut inner = Vec::new();
        let mut prev = None;
        // The most line breaks before a `,` dropped since the last item, which
        // count as line breaks before what follows.
        let mut dropped = 0;
        for item in items {
            let Child {
                element,
                newlines,
            } = item;
            let newlines = newlines.max(mem::take(&mut dropped));
            let kind = ast::kind_of(&element);
            let separator = || match prev {
                None => before_first(),
                Some(_) if newlines > 1 => empty_line(),
                // After a comment, the container is expanded, so the line
                // break comes anyway.
                Some(_) => soft_line_break_or_space(),
            };
            match &element {
                NodeOrToken::Node(Node::RecordField(field)) => {
                    inner.push(separator());
                    inner.extend(self.field(field));
                },
                NodeOrToken::Token(_) if kind == SyntaxKind::Comma => {
                    dropped = newlines;
                    continue;
                },
                NodeOrToken::Token(child) if kind == SyntaxKind::Comment => {
                    expand = true;
                    if prev.is_some() && newlines > 1 {
                        inner.push(empty_line());
                    }
                    inner.extend(self.comment(child, newlines, false));
                },
                _ => {
                    inner.push(separator());
                    inner.extend(self.element(&element));
                },
            }
            prev = Some(kind);
        }
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

    /// A key, its `:` right after it, and its value one space after that.
    /// What follows a comment inside starts a line. A `,` is dropped.
    fn field(&mut self, field: &RecordField) -> Vec<Element<'a>> {
        let mut out = Vec::new();
        let mut prev = None;
        // Parts printed, comments and commas aside; the key is the first.
        let mut parts = 0;
        for child in children(field.syntax().children_with_tokens()) {
            let Child {
                element,
                newlines,
            } = child;
            let kind = ast::kind_of(&element);
            match &element {
                NodeOrToken::Token(_) if kind == SyntaxKind::Comma => continue,
                NodeOrToken::Token(child) if kind == SyntaxKind::Comment => {
                    out.extend(self.comment(child, newlines, false));
                },
                _ => {
                    match prev {
                        None => {},
                        Some(SyntaxKind::Comment) => out.push(hard_line_break()),
                        Some(_) if parts == 1 && kind == SyntaxKind::Colon => {},
                        Some(_) => out.push(space()),
                    }
                    out.extend(self.element(&element));
                    parts += 1;
                },
            }
            prev = Some(kind);
        }
        out
    }
}

/// Whether `container` stays as it is: a comment inside ending in a lone
/// `\r` keeps it so, since the line break printed after the comment would
/// make that `\r` part of the line break.
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
