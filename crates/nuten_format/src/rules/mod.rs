//! Turns each node of the tree into document elements.

mod block;
mod container;
mod control;
mod def;
mod match_;
mod pipeline;
mod signature;

use std::mem;

use nuten_doc::{
    Element, GroupId, empty_line, empty_lines, hard_line_break, line_suffix, space, text,
};
use nuten_syntax::{
    NodeOrToken, SyntaxElement, SyntaxKind, SyntaxNode, SyntaxToken, TextRange,
    ast::{self, AstNode, Node},
};

/// A child of a container: neither whitespace nor a line break.
struct Child {
    element: ast::Element,
    /// Line breaks right before it; two or more make a blank line.
    newlines: u32,
}

impl Child {
    fn kind(&self) -> SyntaxKind {
        ast::kind_of(&self.element)
    }

    fn is_comment(&self) -> bool {
        self.kind() == SyntaxKind::Comment
    }
}

/// The children among `elements`, in source order. Line breaks at the end
/// belong to no child, so nothing is printed for them.
fn children(elements: impl IntoIterator<Item = SyntaxElement>) -> impl Iterator<Item = Child> {
    let mut newlines = 0;
    elements
        .into_iter()
        .filter_map(move |element| match element.kind() {
            SyntaxKind::Whitespace => None,
            SyntaxKind::Newline => {
                newlines += 1;
                None
            },
            _ => {
                let child = Child {
                    element: ast::typed(element),
                    newlines,
                };
                newlines = 0;
                Some(child)
            },
        })
}

/// `children` with every `,` dropped. The child after one or more dropped
/// `,` counts the most line breaks any of them, or itself, had before it.
fn without_commas(children: impl Iterator<Item = Child>) -> impl Iterator<Item = Child> {
    let mut dropped = 0;
    children.filter_map(move |child| {
        let newlines = child.newlines.max(mem::take(&mut dropped));
        if child.kind() == SyntaxKind::Comma {
            dropped = newlines;
            return None;
        }
        Some(Child {
            element: child.element,
            newlines,
        })
    })
}

/// `root` is the tree of `src`, with no `Error` element.
pub(crate) fn file<'a>(src: &'a str, root: &SyntaxNode) -> Vec<Element<'a>> {
    let mut formatter = Formatter {
        src,
        next_group: 0,
    };
    let mut out = formatter.statements(children(root.children_with_tokens()), true);
    out.push(hard_line_break());
    out
}

/// What every rule reads while a file is formatted: the source the elements
/// borrow their text from, and how many group ids have been given out.
struct Formatter<'a> {
    src: &'a str,
    next_group: u32,
}

impl<'a> Formatter<'a> {
    /// Statements; line breaks before the first are left to the container.
    /// Blank lines stay, one at most, or two after a statement ending in `|`.
    /// A comment stays on its own line or at the end of its line; `at_start`
    /// is whether nothing comes before the first child on its line.
    fn statements(
        &mut self,
        items: impl IntoIterator<Item = Child>,
        at_start: bool,
    ) -> Vec<Element<'a>> {
        let mut out = Vec::new();
        let mut prev = None;
        // Whether a `|` at the end of the last statement still reaches what
        // comes next.
        let mut open_pipe = false;
        for item in items {
            let Child {
                element,
                newlines,
            } = item;
            let kind = ast::kind_of(&element);
            if newlines > 0 && prev.is_some() {
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
                NodeOrToken::Node(child) => {
                    if prev == Some(SyntaxKind::Semicolon) && newlines == 0 {
                        out.push(space());
                    }
                    out.extend(self.node(child));
                    open_pipe = ends_with_pipe(child.syntax());
                },
                NodeOrToken::Token(child) => match kind {
                    SyntaxKind::Semicolon => out.push(self.token(child)),
                    SyntaxKind::Comment => {
                        out.extend(self.comment(child, newlines, at_start && prev.is_none()));
                    },
                    kind => unreachable!("{kind:?} is not among statements"),
                },
            }
            prev = Some(kind);
        }
        out
    }

    /// The entry for every node but Root and those their parent lays out
    /// itself.
    fn node(&mut self, node: &Node) -> Vec<Element<'a>> {
        match node {
            Node::Pipeline(pipeline) => self.pipeline(pipeline),
            Node::PipelineElement(_)
            | Node::Call(_)
            | Node::ExternalCall(_)
            | Node::Redirection(_)
            | Node::Match(_) => self.spaced(node.syntax()),
            Node::If(_) | Node::For(_) | Node::While(_) | Node::Loop(_) | Node::Try(_) => {
                self.bodies(node.syntax())
            },
            Node::MatchArms(arms) => self.container(arms),
            Node::MatchArm(arm) => self.arm(arm),
            Node::Def(def) => self.def(def),
            Node::List(list) => self.container(list),
            Node::Record(record) => self.container(record),
            Node::RecordField(field) => self.field(field),
            Node::Signature(signature) => self.container(signature),
            Node::Table(table) => self.table(table),
            Node::Param(param) => self.param(param),
            Node::Type(type_) => self.type_(type_),
            Node::TypeField(field) => self.type_field(field),
            Node::Block(block) => self.block(block),
            Node::Root(_) => unreachable!("Root is laid out by file"),
            Node::Assignment(_) | Node::Let(_) | Node::ClosureParams(_) => {
                unreachable!("{:?} is laid out by its parent", node.syntax().kind())
            },
            Node::Error(_) => unreachable!("a file with errors is not formatted"),
        }
    }

    /// The children with one space between each two. No line break falls
    /// among them and no node ends with trivia, so a comment, which runs to
    /// the end of its line, cannot be here either.
    fn spaced(&mut self, parent: &SyntaxNode) -> Vec<Element<'a>> {
        let items: Vec<_> = children(parent.children_with_tokens()).collect();
        debug_assert!(
            items
                .iter()
                .all(|item| item.newlines == 0 && !item.is_comment()),
            "{:?} holds a line break or a comment",
            parent.kind()
        );
        self.spaced_children(&items, |_, _| false)
    }

    /// `items` one space apart, but those `joined`, by index and by
    /// themselves, which touch what comes before them. A comment stays where
    /// it is, on its own line or at the end of its line, and what follows it
    /// starts a line, joined or not.
    fn spaced_children(
        &mut self,
        items: &[Child],
        joined: impl Fn(usize, &Child) -> bool,
    ) -> Vec<Element<'a>> {
        items
            .iter()
            .enumerate()
            .flat_map(|(index, item)| {
                let prev = items[..index].last().map(Child::kind);
                match &item.element {
                    NodeOrToken::Token(comment) if comment.kind() == SyntaxKind::Comment => {
                        self.comment(comment, item.newlines, false)
                    },
                    element => {
                        let plain = (!joined(index, item)).then(space);
                        separator(prev, plain)
                            .into_iter()
                            .chain(self.element(element))
                            .collect()
                    },
                }
            })
            .collect()
    }

    /// A node as `node` lays it out, or a token as it is.
    fn element(&mut self, element: &ast::Element) -> Vec<Element<'a>> {
        match element {
            NodeOrToken::Node(node) => self.node(node),
            NodeOrToken::Token(token) => vec![self.token(token)],
        }
    }

    /// The one place a comment is printed, other than inside text kept as it
    /// is. `at_start` is whether nothing comes before it on its line.
    fn comment(&self, comment: &SyntaxToken, newlines: u32, at_start: bool) -> Vec<Element<'a>> {
        if newlines > 0 || at_start {
            // The container may have pushed a line break already; the printer
            // keeps the longer of the two.
            vec![hard_line_break(), self.token(comment)]
        } else {
            // A line suffix, so that it does not count toward the width of the
            // line.
            vec![line_suffix([space(), self.token(comment)])]
        }
    }

    /// A token as it is in the source.
    fn token(&self, token: &SyntaxToken) -> Element<'a> {
        self.source(token.text_range())
    }

    /// The source in `range` as it is, trivia included.
    fn source(&self, range: TextRange) -> Element<'a> {
        text(&self.src[range])
    }

    /// An id no other group of this file has.
    fn group_id(&mut self) -> GroupId {
        let id = GroupId(self.next_group);
        self.next_group += 1;
        id
    }
}

/// Whether the statement ends with a `|`; statements never end with
/// trivia, so the last child tells.
fn ends_with_pipe(statement: &SyntaxNode) -> bool {
    statement
        .last_child_or_token()
        .is_some_and(|last| last.kind() == SyntaxKind::Pipe)
}

/// What separates the child after `prev` from it: `plain`, but a line break
/// after a comment, which runs to the end of its line.
fn separator(
    prev: Option<SyntaxKind>,
    plain: Option<Element<'static>>,
) -> Option<Element<'static>> {
    match prev {
        None => None,
        Some(SyntaxKind::Comment) => Some(hard_line_break()),
        Some(_) => plain,
    }
}
