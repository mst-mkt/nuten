//! Turns each node of the tree into document elements.

use std::iter;

use nuten_doc::{
    Element, empty_line, empty_lines, expand_parent, group, hard_line_break, line_suffix,
    soft_line_break_or_space, space, text,
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
    for child in children(pipeline) {
        let Child {
            element,
            newlines,
        } = child;
        let kind = element.kind();
        if newlines > 0 {
            out.push(expand_parent());
        }
        tail = None;
        match &element {
            SyntaxElement::Node(child) => {
                tail = Some(out.len());
                out.extend(separator(prev, space));
                out.extend(node(src, child));
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

/// The one place a comment outside a list is printed. `at_start` is whether
/// it is the first child of its container.
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
