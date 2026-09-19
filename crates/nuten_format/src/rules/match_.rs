use std::mem;

use nuten_doc::{Element, empty_line, hard_line_break, indent, space};
use nuten_syntax::{SyntaxElement, SyntaxKind, SyntaxNode};

use super::{Child, Formatter, children, separator};

impl<'a> Formatter<'a> {
    /// The arms of a `match`, one per line, indented, with the closing brace
    /// on a line of its own; `{}` without any. A `,` is dropped. A blank line
    /// between arms stays, one at most, while those at the start and the end
    /// go. A comment stays where it is, on the line of the opening brace too.
    pub(super) fn arms(&mut self, arms: &SyntaxNode) -> Vec<Element<'a>> {
        let mut items = children(arms);
        let Some(Child {
            element: SyntaxElement::Token(open),
            ..
        }) = items.next()
        else {
            unreachable!("arms have braces");
        };
        let mut body: Vec<_> = items.collect();
        let Some(Child {
            element: SyntaxElement::Token(close),
            ..
        }) = body.pop()
        else {
            unreachable!("arms have braces");
        };
        let mut out = vec![self.token(&open)];
        let mut inner = Vec::new();
        let mut prev = None;
        // Line breaks before a dropped `,` count as before what follows it.
        let mut dropped = 0;
        for child in body {
            let Child {
                element,
                newlines,
            } = child;
            let newlines = newlines.max(mem::take(&mut dropped));
            let kind = element.kind();
            if kind == SyntaxKind::Comma {
                dropped = newlines;
                continue;
            }
            // A comment right after the brace or an arm stays on its line.
            if kind != SyntaxKind::Comment || (newlines > 0 && prev.is_some()) {
                inner.push(if newlines > 1 && prev.is_some() {
                    empty_line()
                } else {
                    hard_line_break()
                });
            }
            match &element {
                SyntaxElement::Node(arm) => inner.extend(self.node(arm)),
                SyntaxElement::Token(child) if kind == SyntaxKind::Comment => {
                    inner.extend(self.comment(child, newlines, false));
                },
                SyntaxElement::Token(_) => unreachable!("{kind:?} is not among arms"),
            }
            prev = Some(kind);
        }
        if !inner.is_empty() {
            out.push(indent(inner));
            out.push(hard_line_break());
        }
        out.push(self.token(&close));
        out
    }

    /// An arm: the tokens of its pattern one space apart, whatever line
    /// breaks came among them, ` => `, and what it does, a block as the block
    /// rule lays it out. A comment stays, and what follows it starts a line.
    pub(super) fn arm(&mut self, arm: &SyntaxNode) -> Vec<Element<'a>> {
        let mut out = Vec::new();
        let mut prev = None;
        for child in children(arm) {
            let Child {
                element,
                newlines,
            } = child;
            let kind = element.kind();
            match &element {
                SyntaxElement::Token(child) if kind == SyntaxKind::Comment => {
                    out.extend(self.comment(child, newlines, false));
                },
                _ => {
                    out.extend(separator(prev, space));
                    out.extend(self.element(&element));
                },
            }
            prev = Some(kind);
        }
        out
    }
}
