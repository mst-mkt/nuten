use std::mem;

use nuten_doc::{
    Element, empty_line, expand_parent, group, hard_line_break, indent, soft_line_break,
    soft_line_break_or_space, space, text,
};
use nuten_syntax::{SyntaxElement, SyntaxKind, SyntaxNode};

use super::{Child, children, comment, node, token};

/// A list, a record, a table, or the parameters of a `def`, unless it stays
/// as it is. On one line when it fits, no line break comes right after the
/// opening bracket, and no comment is inside, with no space inside the
/// brackets of a list and one inside those of a record; otherwise one item
/// per line, indented, with the closing bracket on a line of its own. A table
/// is laid out as a list of its rows, but its column names and, glued to
/// them, their `;` stay on the line of the opening bracket, one space before
/// the rows on one line, and a line break right after the `;` expands it too.
/// A `,` is dropped. A blank line between items stays.
pub(super) fn container<'a>(src: &'a str, container: &SyntaxNode) -> Vec<Element<'a>> {
    if stays_as_is(container) {
        return vec![text(&src[container.text_range()])];
    }
    let (Some(open), Some(close)) = (container.first_token(), container.last_token()) else {
        unreachable!("a container has brackets");
    };
    let mut children = children(container).skip(1).peekable();
    // A line break right after the opening bracket.
    let mut expand = children.peek().is_some_and(|child| child.newlines > 0);
    let mut head = vec![token(src, &open)];
    let closing = if container.kind() == SyntaxKind::Record {
        soft_line_break_or_space
    } else {
        soft_line_break
    };
    let opening = if container.kind() == SyntaxKind::Table {
        // The parser puts the column names and their `;` first, with nothing
        // between.
        let (Some(names), Some(semicolon)) = (children.next(), children.next()) else {
            unreachable!("a table has column names and a `;`");
        };
        match &names.element {
            SyntaxElement::Node(names) => head.extend(node(src, names)),
            SyntaxElement::Token(names) => head.push(token(src, names)),
        }
        let SyntaxElement::Token(semicolon) = &semicolon.element else {
            unreachable!("a table has a `;` after its column names");
        };
        head.push(token(src, semicolon));
        expand |= children.peek().is_some_and(|child| child.newlines > 0);
        soft_line_break_or_space
    } else {
        closing
    };
    let mut inner = Vec::new();
    let mut prev = None;
    // The most line breaks before a `,` dropped since the last item, which
    // count as line breaks before what follows.
    let mut dropped = 0;
    for child in children {
        let Child {
            element,
            newlines,
        } = child;
        let newlines = newlines.max(mem::take(&mut dropped));
        let kind = element.kind();
        let separator = || match prev {
            None => opening(),
            Some(_) if newlines > 1 => empty_line(),
            // After a comment, the container is expanded, so the line break
            // comes anyway.
            Some(_) => soft_line_break_or_space(),
        };
        match &element {
            SyntaxElement::Node(child) if kind == SyntaxKind::RecordField => {
                inner.push(separator());
                inner.extend(field(src, child));
            },
            SyntaxElement::Node(child) => {
                inner.push(separator());
                inner.extend(node(src, child));
            },
            SyntaxElement::Token(_)
                if matches!(kind, SyntaxKind::RBracket | SyntaxKind::RBrace) =>
            {
                break;
            },
            SyntaxElement::Token(_) if kind == SyntaxKind::Comma => {
                dropped = newlines;
                continue;
            },
            SyntaxElement::Token(child) if kind == SyntaxKind::Comment => {
                expand = true;
                if prev.is_some() && newlines > 1 {
                    inner.push(empty_line());
                }
                inner.extend(comment(src, child, newlines, false));
            },
            SyntaxElement::Token(child) => {
                inner.push(separator());
                inner.push(token(src, child));
            },
        }
        prev = Some(kind);
    }
    if inner.is_empty() {
        head.push(token(src, &close));
        return head;
    }
    let mut out = head;
    out.push(indent(inner));
    out.push(closing());
    out.push(token(src, &close));
    if expand {
        out.push(expand_parent());
    }
    vec![group(out)]
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

/// A key, its `:` right after it, and its value one space after that. What
/// follows a comment inside starts a line. A `,` is dropped.
fn field<'a>(src: &'a str, field: &SyntaxNode) -> Vec<Element<'a>> {
    let mut out = Vec::new();
    let mut prev = None;
    // Parts printed, comments and commas aside; the key is the first.
    let mut parts = 0;
    for child in children(field) {
        let Child {
            element,
            newlines,
        } = child;
        let kind = element.kind();
        match &element {
            SyntaxElement::Token(_) if kind == SyntaxKind::Comma => continue,
            SyntaxElement::Token(child) if kind == SyntaxKind::Comment => {
                out.extend(comment(src, child, newlines, false));
            },
            _ => {
                match prev {
                    None => {},
                    Some(SyntaxKind::Comment) => out.push(hard_line_break()),
                    Some(_) if parts == 1 && kind == SyntaxKind::Colon => {},
                    Some(_) => out.push(space()),
                }
                match &element {
                    SyntaxElement::Node(child) => out.extend(node(src, child)),
                    SyntaxElement::Token(child) => out.push(token(src, child)),
                }
                parts += 1;
            },
        }
        prev = Some(kind);
    }
    out
}
