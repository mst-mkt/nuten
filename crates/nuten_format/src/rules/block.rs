use nuten_doc::{
    Element, empty_line, expand_parent, group, indent, soft_line_break_or_space, text,
};
use nuten_syntax::{SyntaxElement, SyntaxKind, SyntaxNode};

use super::{Child, children, statements, token};

/// Whether `block` is an argument of `match`, its subject or its arms where
/// nu reads no arms, which stay as they are.
pub(super) fn in_match(block: &SyntaxNode) -> bool {
    block
        .parent()
        .is_some_and(|parent| parent.kind() == SyntaxKind::Match)
}

/// A block or a closure. The parameters come right after the opening brace,
/// as they are. The rest goes on one line when it fits and no line break
/// comes after the parameters, with one space inside the braces; otherwise
/// the statements go indented, with the closing brace on a line of its own.
/// Blank lines at the end go, and so do those at the start, unless a comment
/// comes before them.
pub(super) fn block<'a>(src: &'a str, block: &SyntaxNode) -> Vec<Element<'a>> {
    let BlockLayout {
        mut head,
        mut rest,
        line_break,
        ..
    } = block_layout(src, block);
    if line_break {
        rest.push(expand_parent());
    }
    head.push(group(rest));
    head
}

/// A block laid out for its container to group.
pub(super) struct BlockLayout<'a> {
    /// The opening brace and the parameters, which stay out of the group, so
    /// that parameters over lines leave the rest to fit after their last
    /// line.
    pub(super) head: Vec<Element<'a>>,
    pub(super) rest: Vec<Element<'a>>,
    /// Whether a line break comes after the parameters in the source.
    line_break: bool,
    /// Whether it holds at most one statement and nothing else.
    pub(super) one_statement: bool,
}

pub(super) fn block_layout<'a>(src: &'a str, block: &SyntaxNode) -> BlockLayout<'a> {
    let mut items = children(block).peekable();
    let Some(Child {
        element: SyntaxElement::Token(open),
        ..
    }) = items.next()
    else {
        unreachable!("a block has braces");
    };
    let mut head = vec![token(src, &open)];
    let params = items.next_if(|item| item.element.kind() == SyntaxKind::ClosureParams);
    if let Some(params) = &params {
        head.push(text(&src[params.element.text_range()]));
    }
    let mut body: Vec<_> = items.collect();
    let Some(Child {
        element: SyntaxElement::Token(close),
        newlines: close_newlines,
    }) = body.pop()
    else {
        unreachable!("a block has braces");
    };
    let one_statement = matches!(
        body.as_slice(),
        [] | [Child {
            element: SyntaxElement::Node(_),
            ..
        }]
    );
    let Some(first) = body.first() else {
        return BlockLayout {
            head,
            rest: vec![token(src, &close)],
            line_break: false,
            one_statement,
        };
    };
    // A comment is followed by a line break, so it counts too.
    let line_break = close_newlines > 0 || body.iter().any(|item| item.newlines > 0);
    let lead = if first.element.kind() == SyntaxKind::Comment && first.newlines == 0 {
        None
    } else if params.is_none()
        && first
            .element
            .as_node()
            .and_then(SyntaxNode::first_child_or_token)
            .is_some_and(|first| first.kind() == SyntaxKind::Pipe)
    {
        // Right after the brace and a line break, nu would read the `|` as
        // the start of parameters.
        Some(empty_line())
    } else {
        Some(soft_line_break_or_space())
    };
    let inner = lead.into_iter().chain(statements(src, body, false));
    BlockLayout {
        head,
        rest: vec![
            indent(inner),
            soft_line_break_or_space(),
            token(src, &close),
        ],
        line_break,
        one_statement,
    }
}
