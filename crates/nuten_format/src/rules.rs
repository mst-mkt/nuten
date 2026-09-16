//! Turns each node of the tree into document elements.

use nuten_doc::{
    Element, empty_line, empty_lines, expand_parent, group, hard_line_break, line_suffix,
    soft_line_break_or_space, space, text,
};
use nuten_syntax::{SyntaxElement, SyntaxKind, SyntaxNode, SyntaxToken};

/// `root` is the tree of `src`, with no `Error` element.
/// Blank lines stay, one at most, or two after a statement ending in `|`.
/// A comment stays on its own line or at the end of its line.
pub(crate) fn file<'a>(src: &'a str, root: &SyntaxNode) -> Vec<Element<'a>> {
    let mut out = Vec::new();
    // The last child, whitespace aside.
    let mut prev = None;
    // Line breaks in a row, the current child included; two make a blank line.
    let mut newlines = 0u32;
    // Whether a `|` at the end of the last statement still reaches what
    // comes next.
    let mut open_pipe = false;
    for child in root.children_with_tokens() {
        let kind = child.kind();
        if kind == SyntaxKind::Whitespace {
            continue;
        }
        if kind == SyntaxKind::Newline {
            newlines += 1;
        } else {
            if newlines > 1 {
                // A blank line closes the pipe above, as it did in the source.
                open_pipe = false;
            }
            newlines = 0;
        }
        match &child {
            SyntaxElement::Node(child) => {
                if prev == Some(SyntaxKind::Semicolon) {
                    out.push(space());
                }
                out.extend(node(src, child));
                open_pipe = ends_with_pipe(child);
            },
            SyntaxElement::Token(child) => match kind {
                SyntaxKind::Semicolon => out.push(token(src, child)),
                SyntaxKind::Newline => out.push(match newlines {
                    1 => hard_line_break(),
                    // With one blank line, a `|` at the start of the next
                    // line would join the statement above, so two stay.
                    3.. if open_pipe => empty_lines(2),
                    _ => empty_line(),
                }),
                SyntaxKind::Comment => {
                    out.push(if matches!(prev, None | Some(SyntaxKind::Newline)) {
                        token(src, child)
                    } else {
                        // A line suffix, so that it does not count toward the
                        // width of the line.
                        line_suffix([space(), token(src, child)])
                    });
                },
                _ => unreachable!("{kind:?} is not a child of Root"),
            },
        }
        prev = Some(kind);
    }
    while matches!(out.last(), Some(Element::Line(_))) {
        out.pop();
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
        kind => unreachable!("{kind:?} is not formatted as a node"),
    }
}

/// On one line when it fits and was on one line in the source; otherwise
/// each `|` starts a line, at the current indent, while `e>|` and `o+e>|`
/// stay on the line of their element.
/// A comment stays where it is among the pipes and elements, on its own
/// line or at the end of its line. A pipe with a comment after it stays at
/// the end of its line, and what comes next starts a line.
fn pipeline<'a>(src: &'a str, pipeline: &SyntaxNode) -> Vec<Element<'a>> {
    let mut out = Vec::new();
    // The last child, whitespace aside.
    let mut prev = None;
    // The last pipe, element, or comment.
    let mut last = None;
    for child in pipeline.children_with_tokens() {
        let kind = child.kind();
        match &child {
            SyntaxElement::Node(child) => {
                if last == Some(SyntaxKind::Comment) {
                    out.push(hard_line_break());
                } else if !out.is_empty() {
                    out.push(space());
                }
                out.extend(node(src, child));
            },
            SyntaxElement::Token(child) => match kind {
                SyntaxKind::Pipe => {
                    if last == Some(SyntaxKind::Comment) {
                        out.push(hard_line_break());
                    } else if !out.is_empty() {
                        out.push(if comment_follows(child) {
                            space()
                        } else {
                            soft_line_break_or_space()
                        });
                    }
                    out.push(token(src, child));
                },
                // Nu rejects these at the start of a line, so they stay
                // after their element, which the parser puts right before.
                SyntaxKind::ErrGtPipe | SyntaxKind::OutErrGtPipe => {
                    out.push(space());
                    out.push(token(src, child));
                },
                SyntaxKind::Comment => {
                    if prev == Some(SyntaxKind::Newline) {
                        out.push(hard_line_break());
                        out.push(token(src, child));
                    } else {
                        // A line suffix, so that it does not count toward the
                        // width of the line.
                        out.push(line_suffix([space(), token(src, child)]));
                    }
                },
                SyntaxKind::Newline => out.push(expand_parent()),
                SyntaxKind::Whitespace => continue,
                kind => unreachable!("{kind:?} is not a child of Pipeline"),
            },
        }
        prev = Some(kind);
        if kind != SyntaxKind::Newline {
            last = Some(kind);
        }
    }
    vec![group(out)]
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

/// The children, trivia aside, with one space between each two.
fn spaced<'a>(src: &'a str, parent: &SyntaxNode) -> Vec<Element<'a>> {
    let mut out = Vec::new();
    for child in parent
        .children_with_tokens()
        .filter(|child| !child.kind().is_trivia())
    {
        if !out.is_empty() {
            out.push(space());
        }
        match &child {
            SyntaxElement::Node(child) => out.extend(node(src, child)),
            SyntaxElement::Token(child) => out.push(token(src, child)),
        }
    }
    out
}

/// As it is in the source.
fn token<'a>(src: &'a str, token: &SyntaxToken) -> Element<'a> {
    text(&src[token.text_range()])
}
