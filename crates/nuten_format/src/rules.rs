//! Turns each node of the tree into document elements.

use std::{iter, mem};

use nuten_doc::{
    Element, empty_line, empty_lines, expand_parent, group, hard_line_break, indent, line_suffix,
    soft_line_break, soft_line_break_or_space, space, text,
};
use nuten_syntax::{SyntaxElement, SyntaxKind, SyntaxNode, SyntaxToken};

/// A child of a container: neither whitespace nor a line break.
struct Child {
    element: SyntaxElement,
    /// Line breaks right before it; two or more make a blank line.
    newlines: u32,
}

/// The children of `parent` in source order. Line breaks at its end, which
/// only Root has, belong to no child, so nothing is printed for them.
fn children(parent: &SyntaxNode) -> impl Iterator<Item = Child> {
    let mut newlines = 0;
    parent
        .children_with_tokens()
        .filter_map(move |element| match element.kind() {
            SyntaxKind::Whitespace => None,
            SyntaxKind::Newline => {
                newlines += 1;
                None
            },
            _ => {
                let child = Child {
                    element,
                    newlines,
                };
                newlines = 0;
                Some(child)
            },
        })
}

/// `root` is the tree of `src`, with no `Error` element.
/// Blank lines stay, one at most, or two after a statement ending in `|`.
/// A comment stays on its own line or at the end of its line.
pub(crate) fn file<'a>(src: &'a str, root: &SyntaxNode) -> Vec<Element<'a>> {
    let mut out = Vec::new();
    let mut prev = None;
    // Whether a `|` at the end of the last statement still reaches what
    // comes next.
    let mut open_pipe = false;
    for child in children(root) {
        let Child {
            element,
            newlines,
        } = child;
        let kind = element.kind();
        if newlines > 0 {
            out.push(match newlines {
                1 => hard_line_break(),
                // With one blank line, a `|` at the start of the next line
                // would join the statement above, so two stay.
                3.. if open_pipe => empty_lines(2),
                _ => empty_line(),
            });
            // A blank line closes the pipe above, as it did in the source.
            if newlines > 1 {
                open_pipe = false;
            }
        }
        match &element {
            SyntaxElement::Node(child) => {
                if prev == Some(SyntaxKind::Semicolon) && newlines == 0 {
                    out.push(space());
                }
                out.extend(node(src, child));
                open_pipe = ends_with_pipe(child);
            },
            SyntaxElement::Token(child) => match kind {
                SyntaxKind::Semicolon => out.push(token(src, child)),
                SyntaxKind::Comment => out.extend(comment(src, child, newlines, prev.is_none())),
                kind => unreachable!("{kind:?} is not a child of Root"),
            },
        }
        prev = Some(kind);
    }
    out.push(hard_line_break());
    out
}

/// Whether the statement ends with a `|`; statements never end with
/// trivia, so the last child tells.
fn ends_with_pipe(statement: &SyntaxNode) -> bool {
    statement
        .last_child_or_token()
        .is_some_and(|last| last.kind() == SyntaxKind::Pipe)
}

/// The entry for every node but Root.
fn node<'a>(src: &'a str, node: &SyntaxNode) -> Vec<Element<'a>> {
    match node.kind() {
        SyntaxKind::Pipeline => pipeline(src, node),
        SyntaxKind::PipelineElement
        | SyntaxKind::Call
        | SyntaxKind::ExternalCall
        | SyntaxKind::Redirection => spaced(src, node),
        SyntaxKind::List if reads_as_list(node) => list(src, node),
        SyntaxKind::List | SyntaxKind::Record | SyntaxKind::Block => {
            vec![text(&src[node.text_range()])]
        },
        kind => unreachable!("{kind:?} is not formatted as a node"),
    }
}

/// On one line when no line break came before a pipe or element in the
/// source and it fits, counting only the first line of an element that ends
/// it; otherwise each `|` starts a line, at the current indent, while `e>|`
/// and `o+e>|` stay on the line of their element.
/// A comment stays where it is among the pipes and elements, on its own
/// line or at the end of its line. A pipe with a comment after it stays at
/// the end of its line, and what comes next starts a line.
fn pipeline<'a>(src: &'a str, pipeline: &SyntaxNode) -> Vec<Element<'a>> {
    let mut out = Vec::new();
    let mut prev = None;
    // Where an element that ends the pipeline, with its separator, starts in
    // `out`.
    let mut tail = None;
    for part in parts(pipeline) {
        let Part {
            child: Child {
                element,
                newlines,
            },
            joined,
        } = part;
        let kind = element.kind();
        if newlines > 0 {
            out.push(expand_parent());
        }
        if joined {
            out.push(space());
            match &element {
                SyntaxElement::Node(child) => out.extend(node(src, child)),
                SyntaxElement::Token(child) => out.push(token(src, child)),
            }
            prev = Some(kind);
            continue;
        }
        tail = None;
        match &element {
            SyntaxElement::Node(child) => {
                tail = Some(out.len());
                out.extend(separator(prev, space));
                out.extend(node(src, child));
            },
            // `let`, or an assignment operator with nothing before it, which
            // starts an element spread out.
            SyntaxElement::Token(child)
                if matches!(kind, SyntaxKind::Word | SyntaxKind::Assign) =>
            {
                tail = Some(out.len());
                out.extend(separator(prev, space));
                out.push(token(src, child));
            },
            SyntaxElement::Token(child) => match kind {
                SyntaxKind::Pipe => {
                    out.extend(separator(prev, || {
                        if comment_follows(child) {
                            space()
                        } else {
                            soft_line_break_or_space()
                        }
                    }));
                    out.push(token(src, child));
                },
                // Nu rejects these at the start of a line, so they stay
                // after their element, which the parser puts right before.
                SyntaxKind::ErrGtPipe | SyntaxKind::OutErrGtPipe => {
                    out.extend(separator(prev, space));
                    out.push(token(src, child));
                },
                SyntaxKind::Comment => out.extend(comment(src, child, newlines, prev.is_none())),
                kind => unreachable!("{kind:?} is not a child of Pipeline"),
            },
        }
        prev = Some(kind);
    }
    // The tail is measured in break mode, so its first line break ends the
    // measure instead of breaking the group.
    let tail = out.split_off(tail.unwrap_or(out.len()));
    if out.is_empty() {
        return tail;
    }
    iter::once(group(out)).chain(tail).collect()
}

/// Whether nu reads `list` as a list whatever the command: as the head of a
/// command, or as an element of a list. Elsewhere, such as an argument, a
/// command may read it as a string.
fn reads_as_list(list: &SyntaxNode) -> bool {
    list.parent().is_some_and(|parent| match parent.kind() {
        SyntaxKind::List => true,
        SyntaxKind::Call => {
            parent
                .first_child_or_token()
                .and_then(SyntaxElement::into_node)
                == Some(list.clone())
        },
        _ => false,
    })
}

/// On one line, with no space inside the brackets, when it fits, no line
/// break comes right after `[`, and no comment is inside; otherwise one
/// element per line, indented, with `]` on a line of its own. A `,` is
/// dropped. A blank line between elements stays.
fn list<'a>(src: &'a str, list: &SyntaxNode) -> Vec<Element<'a>> {
    let (Some(open), Some(close)) = (list.first_token(), list.last_token()) else {
        unreachable!("a list has brackets");
    };
    let mut children = children(list).skip(1).peekable();
    // A line break right after `[`.
    let mut expand = children.peek().is_some_and(|child| child.newlines > 0);
    let mut inner = Vec::new();
    let mut prev = None;
    // The most line breaks before a `,` dropped since the last element, which
    // count as line breaks before what follows.
    let mut dropped = 0;
    for child in children {
        let Child {
            element,
            newlines,
        } = child;
        let newlines = newlines.max(mem::take(&mut dropped));
        let kind = element.kind();
        match &element {
            SyntaxElement::Node(child) => {
                inner.push(list_separator(prev, newlines));
                inner.extend(node(src, child));
            },
            SyntaxElement::Token(_) if kind == SyntaxKind::RBracket => break,
            SyntaxElement::Token(_) if kind == SyntaxKind::Comma => {
                dropped = newlines;
                continue;
            },
            SyntaxElement::Token(child) if kind == SyntaxKind::Comment => {
                if keeps_list_as_is(child) {
                    return vec![text(&src[list.text_range()])];
                }
                expand = true;
                if prev.is_some() && newlines > 1 {
                    inner.push(empty_line());
                }
                inner.extend(comment(src, child, newlines, false));
            },
            SyntaxElement::Token(child) => {
                inner.push(list_separator(prev, newlines));
                inner.push(token(src, child));
            },
        }
        prev = Some(kind);
    }
    if inner.is_empty() {
        return vec![token(src, &open), token(src, &close)];
    }
    let mut out = vec![
        token(src, &open),
        indent(inner),
        soft_line_break(),
        token(src, &close),
    ];
    if expand {
        out.push(expand_parent());
    }
    vec![group(out)]
}

/// What comes before an element of a list after `prev`. After a comment,
/// the list is expanded, so the line break comes anyway.
fn list_separator(prev: Option<SyntaxKind>, newlines: u32) -> Element<'static> {
    match prev {
        None => soft_line_break(),
        Some(_) if newlines > 1 => empty_line(),
        Some(_) => soft_line_break_or_space(),
    }
}

/// Whether `comment` makes its list stay as it is. Right after anything but
/// whitespace, nu does not start a comment when it reads the word the list is
/// in, so a space put before it may move where the word ends. A lone `\r` at
/// its end would become part of a line break.
fn keeps_list_as_is(comment: &SyntaxToken) -> bool {
    comment.text().ends_with('\r')
        || comment
            .prev_sibling_or_token()
            .is_some_and(|prev| !prev.kind().is_trivia())
}

/// A child of a pipeline as `pipeline` formats it.
struct Part {
    child: Child,
    /// Whether it belongs to the element before it, one space after.
    joined: bool,
}

/// The children of `pipeline`, with each element that holds an `Assignment`
/// or a `Let` spread out: its parts up to the operator, then any comment,
/// then the children of the pipeline it takes, whose first element joins
/// the operator unless a line break or a comment comes between. So the
/// pipes of what the operator takes format as those of `pipeline`.
fn parts(pipeline: &SyntaxNode) -> Vec<Part> {
    let mut out = Vec::new();
    spread(pipeline, 0, false, &mut out);
    out
}

/// `newlines` come before the first child, and `joined` is whether it joins
/// what comes before when it is an element.
fn spread(pipeline: &SyntaxNode, newlines: u32, joined: bool, out: &mut Vec<Part>) {
    let mut first = Some((newlines, joined));
    for child in children(pipeline) {
        let (before, joined) = first.take().unwrap_or((0, false));
        let newlines = before + child.newlines;
        let assignment = child
            .element
            .as_node()
            .and_then(SyntaxNode::first_child)
            .filter(|node| matches!(node.kind(), SyntaxKind::Assignment | SyntaxKind::Let));
        match assignment {
            Some(assignment) => spread_assignment(&assignment, newlines, joined, out),
            None => out.push(Part {
                joined: joined && child.element.as_node().is_some(),
                child: Child {
                    element: child.element,
                    newlines,
                },
            }),
        }
    }
}

/// The parts of `assignment` for `spread`: each joins the part before it but
/// a comment, and the first, which joins as `joined` tells.
fn spread_assignment(assignment: &SyntaxNode, newlines: u32, joined: bool, out: &mut Vec<Part>) {
    let mut first = Some((newlines, joined));
    for child in children(assignment) {
        let (before, joined) = first.take().unwrap_or((0, true));
        let Child {
            element,
            newlines,
        } = child;
        let newlines = before + newlines;
        let kind = element.kind();
        match &element {
            SyntaxElement::Node(pipeline) if kind == SyntaxKind::Pipeline => {
                spread(pipeline, newlines, true, out);
            },
            _ => out.push(Part {
                joined: joined && kind != SyntaxKind::Comment,
                child: Child {
                    element,
                    newlines,
                },
            }),
        }
    }
}

/// What separates the child after `prev` from it. A comment runs to the end
/// of its line, so a line break follows one.
fn separator(
    prev: Option<SyntaxKind>,
    plain: impl FnOnce() -> Element<'static>,
) -> Option<Element<'static>> {
    match prev {
        None => None,
        Some(SyntaxKind::Comment) => Some(hard_line_break()),
        Some(_) => Some(plain()),
    }
}

/// Whether a comment comes before the next pipe or element after `pipe`.
fn comment_follows(pipe: &SyntaxToken) -> bool {
    let mut next = pipe.next_sibling_or_token();
    while let Some(sibling) = &next {
        match sibling.kind() {
            SyntaxKind::Comment => return true,
            SyntaxKind::Whitespace | SyntaxKind::Newline => next = sibling.next_sibling_or_token(),
            _ => return false,
        }
    }
    false
}

/// The children with one space between each two. No line break falls among
/// them and no node ends with trivia, so a comment, which runs to the end of
/// its line, cannot be here either.
fn spaced<'a>(src: &'a str, parent: &SyntaxNode) -> Vec<Element<'a>> {
    let mut out = Vec::new();
    for child in children(parent) {
        debug_assert!(
            child.newlines == 0 && child.element.kind() != SyntaxKind::Comment,
            "{:?} holds a line break or a comment",
            parent.kind()
        );
        if !out.is_empty() {
            out.push(space());
        }
        match &child.element {
            SyntaxElement::Node(child) => out.extend(node(src, child)),
            SyntaxElement::Token(child) => out.push(token(src, child)),
        }
    }
    out
}

/// The one place a comment is printed, other than inside text kept as it is.
/// `at_start` is whether it is the first child of its container.
fn comment<'a>(
    src: &'a str,
    comment: &SyntaxToken,
    newlines: u32,
    at_start: bool,
) -> Vec<Element<'a>> {
    if newlines > 0 || at_start {
        // The container may have pushed a line break already; the printer
        // keeps the longer of the two.
        vec![hard_line_break(), token(src, comment)]
    } else {
        // A line suffix, so that it does not count toward the width of the
        // line.
        vec![line_suffix([space(), token(src, comment)])]
    }
}

/// As it is in the source.
fn token<'a>(src: &'a str, token: &SyntaxToken) -> Element<'a> {
    text(&src[token.text_range()])
}
