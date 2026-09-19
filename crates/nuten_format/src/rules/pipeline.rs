use std::iter;

use nuten_doc::{
    Element, GroupId, align, align_if_group_breaks, expand_parent, group_with_id,
    soft_line_break_or_space, space,
};
use nuten_syntax::{
    NodeOrToken, SyntaxKind, SyntaxNode, SyntaxToken,
    ast::{self, AstNode, Node, Pipeline},
};

use super::{Child, Formatter, children, separator};

impl<'a> Formatter<'a> {
    /// On one line when no line break came before a pipe or element in the
    /// source and it fits, counting only the first line of an element that
    /// ends it; otherwise each `|` starts a line, at the current indent, while
    /// `e>|` and `o+e>|` stay on the line of their element. What follows a
    /// `|` at the start of a line goes two columns further on its lines after
    /// the first, so that it lines up after the `| `.
    /// A comment stays where it is among the pipes and elements, on its own
    /// line or at the end of its line. A pipe with a comment after it stays
    /// at the end of its line, and what comes next starts a line.
    pub(super) fn pipeline(&mut self, pipeline: &Pipeline) -> Vec<Element<'a>> {
        let id = self.group_id();
        let mut out = Vec::new();
        let mut prev = None;
        // Where an element that ends the pipeline, with its separator, starts
        // in `out`.
        let mut tail = None;
        // Where what follows a `|` that may start a line starts in `out`, and
        // whether the `|` starts a line even when the pipeline is on one line.
        let mut after_pipe: Option<(usize, bool)> = None;
        for part in parts(pipeline.syntax()) {
            let Part {
                child:
                    Child {
                        element,
                        newlines,
                    },
                joined,
            } = part;
            let kind = ast::kind_of(&element);
            if matches!(kind, SyntaxKind::Pipe | SyntaxKind::Comment) {
                align_after_pipe(&mut out, after_pipe.take(), id, &mut tail);
            }
            if newlines > 0 {
                out.push(expand_parent());
            }
            if joined {
                out.push(space());
                out.extend(self.element(&element));
                prev = Some(kind);
                continue;
            }
            tail = None;
            match &element {
                NodeOrToken::Node(child) => {
                    tail = Some(out.len());
                    out.extend(separator(prev, Some(space())));
                    out.extend(self.node(child));
                },
                // `let`, or an assignment operator with nothing before it,
                // which starts an element laid flat.
                NodeOrToken::Token(child)
                    if matches!(kind, SyntaxKind::Word | SyntaxKind::Assign) =>
                {
                    tail = Some(out.len());
                    out.extend(separator(prev, Some(space())));
                    out.push(self.token(child));
                },
                NodeOrToken::Token(child) => match kind {
                    SyntaxKind::Pipe => {
                        let before_comment = comment_follows(child);
                        out.extend(separator(
                            prev,
                            Some(if before_comment {
                                space()
                            } else {
                                soft_line_break_or_space()
                            }),
                        ));
                        out.push(self.token(child));
                        // A statement starts a line in the output but right
                        // after a `;` on the same line, where its `|` never
                        // does; in a block on one line, nothing breaks anyway.
                        if !before_comment
                            && (prev.is_some() || !follows_semicolon(pipeline.syntax()))
                        {
                            let starts_line = matches!(prev, None | Some(SyntaxKind::Comment));
                            after_pipe = Some((out.len(), starts_line));
                        }
                    },
                    // Nu rejects these at the start of a line, so they stay
                    // after their element, which the parser puts right before.
                    SyntaxKind::ErrGtPipe | SyntaxKind::OutErrGtPipe => {
                        out.extend(separator(prev, Some(space())));
                        out.push(self.token(child));
                    },
                    SyntaxKind::Comment => {
                        out.extend(self.comment(child, newlines, prev.is_none()));
                    },
                    kind => unreachable!("{kind:?} is not a child of Pipeline"),
                },
            }
            prev = Some(kind);
        }
        align_after_pipe(&mut out, after_pipe, id, &mut tail);
        // The tail is measured in break mode, so its first line break ends the
        // measure instead of breaking the group.
        let tail = out.split_off(tail.unwrap_or(out.len()));
        if out.is_empty() {
            return tail;
        }
        iter::once(group_with_id(id, out)).chain(tail).collect()
    }
}

/// Whether a `;` comes right before `node`, past whitespace.
fn follows_semicolon(node: &SyntaxNode) -> bool {
    let mut before = node.prev_sibling_or_token();
    while let Some(sibling) = &before
        && sibling.kind() == SyntaxKind::Whitespace
    {
        before = sibling.prev_sibling_or_token();
    }
    before.is_some_and(|before| before.kind() == SyntaxKind::Semicolon)
}

/// Aligns `out` from the start in `after_pipe`, right after a `|`: always
/// when that `|` starts a line anyway, otherwise when the group `id` of the
/// pipeline breaks. The part before `tail` is aligned apart, so that it stays
/// in the group.
fn align_after_pipe(
    out: &mut Vec<Element<'_>>,
    after_pipe: Option<(usize, bool)>,
    id: GroupId,
    tail: &mut Option<usize>,
) {
    let Some((start, starts_line)) = after_pipe else {
        return;
    };
    let aligned = |elements| {
        if starts_line {
            align(2, elements)
        } else {
            align_if_group_breaks(id, 2, elements)
        }
    };
    let mut elements = out.split_off(start);
    if let Some(tail) = tail
        && *tail > start
    {
        let rest = elements.split_off(*tail - start);
        out.push(aligned(elements));
        *tail = out.len();
        elements = rest;
    }
    if !elements.is_empty() {
        out.push(aligned(elements));
    }
}

/// A child of a pipeline as `pipeline` formats it.
struct Part {
    child: Child,
    /// Whether it belongs to the element before it, one space after.
    joined: bool,
}

/// The children of `pipeline`, with each element that holds an `Assignment`
/// or a `Let` laid flat: its parts up to the operator, then any comment,
/// then the children of the pipeline it takes, whose first element joins
/// the operator unless a line break or a comment comes between. So the
/// pipes of what the operator takes format as those of `pipeline`.
fn parts(pipeline: &SyntaxNode) -> Vec<Part> {
    let mut out = Vec::new();
    flatten(pipeline, 0, false, &mut out);
    out
}

/// `newlines` come before the first child, and `joined` is whether it joins
/// what comes before when it is an element.
fn flatten(pipeline: &SyntaxNode, newlines: u32, joined: bool, out: &mut Vec<Part>) {
    let mut first = Some((newlines, joined));
    for child in children(pipeline.children_with_tokens()) {
        let (before, joined) = first.take().unwrap_or((0, false));
        let newlines = before + child.newlines;
        let assignment = child
            .element
            .as_node()
            .and_then(|node| node.syntax().first_child())
            .filter(|node| matches!(node.kind(), SyntaxKind::Assignment | SyntaxKind::Let));
        match assignment {
            Some(assignment) => flatten_assignment(&assignment, newlines, joined, out),
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

/// The parts of `assignment` for `flatten`: each joins the part before it but
/// a comment, and the first, which joins as `joined` tells.
fn flatten_assignment(assignment: &SyntaxNode, newlines: u32, joined: bool, out: &mut Vec<Part>) {
    let mut first = Some((newlines, joined));
    for child in children(assignment.children_with_tokens()) {
        let (before, joined) = first.take().unwrap_or((0, true));
        let Child {
            element,
            newlines,
        } = child;
        let newlines = before + newlines;
        match &element {
            NodeOrToken::Node(Node::Pipeline(pipeline)) => {
                flatten(pipeline.syntax(), newlines, true, out);
            },
            _ => out.push(Part {
                joined: joined && ast::kind_of(&element) != SyntaxKind::Comment,
                child: Child {
                    element,
                    newlines,
                },
            }),
        }
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
