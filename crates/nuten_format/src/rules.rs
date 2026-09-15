//! Turns each node of the tree into document elements.

use nuten_doc::{
    Element, empty_line, expand_parent, group, hard_line_break, line_suffix,
    soft_line_break_or_space, space, text,
};
use nuten_syntax::{SyntaxElement, SyntaxKind, SyntaxNode, SyntaxToken};

/// `root` is the tree of `src`, with no `Error` element.
/// Blank lines stay, one at most.
/// A comment stays on its own line or at the end of its line.
pub(crate) fn file<'a>(src: &'a str, root: &SyntaxNode) -> Vec<Element<'a>> {
    let mut out = Vec::new();
    // The last child, whitespace aside.
    let mut prev = None;
    for child in root.children_with_tokens() {
        match &child {
            SyntaxElement::Node(child) => {
                if prev == Some(SyntaxKind::Semicolon) {
                    out.push(space());
                }
                out.extend(node(src, child));
            },
            SyntaxElement::Token(child) => match child.kind() {
                SyntaxKind::Semicolon => out.push(token(src, child)),
                SyntaxKind::Newline => out.push(if prev == Some(SyntaxKind::Newline) {
                    empty_line()
                } else {
                    hard_line_break()
                }),
                SyntaxKind::Whitespace => continue,
                SyntaxKind::Comment => {
                    out.push(if matches!(prev, None | Some(SyntaxKind::Newline)) {
                        token(src, child)
                    } else {
                        // A line suffix, so that it does not count toward the
                        // width of the line.
                        line_suffix([space(), token(src, child)])
                    });
                },
                kind => unreachable!("{kind:?} is not a child of Root"),
            },
        }
        prev = Some(child.kind());
    }
    while matches!(out.last(), Some(Element::Line(_))) {
        out.pop();
    }
    out.push(hard_line_break());
    out
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
/// each pipe starts a line, at the current indent.
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
                SyntaxKind::Pipe | SyntaxKind::ErrGtPipe | SyntaxKind::OutErrGtPipe => {
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
