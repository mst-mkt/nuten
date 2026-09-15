//! Turns each node of the tree into document elements.

use nuten_doc::{Element, empty_line, hard_line_break, line_suffix, space, text};
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
        SyntaxKind::Pipeline
        | SyntaxKind::PipelineElement
        | SyntaxKind::Call
        | SyntaxKind::ExternalCall
        | SyntaxKind::Redirection => spaced(src, node),
        kind => unreachable!("{kind:?} is not formatted as a node"),
    }
}

/// The children, trivia aside, with one space between each two.
/// A pipeline goes on one line this way, and loses its comments, until the
/// pipeline rules land.
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
