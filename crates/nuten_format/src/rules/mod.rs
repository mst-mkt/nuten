//! Turns each node of the tree into document elements.

mod binary;
mod block;
mod container;
mod control;
mod def;
mod match_;
mod pipeline;
mod signature;
mod subexpression;

use std::mem;

use nuten_doc::{
    Element, GroupIds, empty_line, empty_lines, hard_line_break, line_suffix, space, text,
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

    fn as_comment(&self) -> Option<&SyntaxToken> {
        self.element
            .as_token()
            .filter(|token| token.kind() == SyntaxKind::Comment)
    }

    fn is_comment(&self) -> bool {
        self.as_comment().is_some()
    }

    fn is_statement(&self) -> bool {
        self.kind() == SyntaxKind::Pipeline
    }
}

/// The children among `elements`, in source order.
/// Line breaks at the end belong to no child, so nothing is printed for them.
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
        groups: GroupIds::default(),
    };
    let statements: Vec<_> = children(root.children_with_tokens()).collect();
    let mut out = formatter.statements(&statements, true);
    out.push(hard_line_break());
    out
}

/// What every rule reads while a file is formatted.
/// The source the elements borrow their text from, and the group ids given out so far.
struct Formatter<'a> {
    src: &'a str,
    groups: GroupIds,
}

impl<'a> Formatter<'a> {
    /// Statements; line breaks before the first are left to the container.
    fn statements(&mut self, items: &[Child], at_start: bool) -> Vec<Element<'a>> {
        items
            .iter()
            .enumerate()
            .flat_map(|(index, item)| self.statement(&items[..index], item, at_start))
            .collect()
    }

    /// The statement, `;`, or comment `item` after what separates it from the last of `before`.
    /// `before` is every child that comes before it.
    fn statement(&mut self, before: &[Child], item: &Child, at_start: bool) -> Vec<Element<'a>> {
        let Child {
            element,
            newlines,
        } = item;
        let prev = before.last().map(Child::kind);
        let separator = match (prev, *newlines) {
            (None, _) | (_, 0) => None,
            (_, 1) => Some(hard_line_break()),
            // With one blank line, a `|` starting the next line would join the statement above.
            // So two stay.
            (_, 3..) if pipe_open(before) => Some(empty_lines(2)),
            _ => Some(empty_line()),
        };
        let item = match element {
            NodeOrToken::Node(child) => {
                let after_semicolon = prev == Some(SyntaxKind::Semicolon) && *newlines == 0;
                after_semicolon
                    .then(space)
                    .into_iter()
                    .chain(self.node(child))
                    .collect()
            },
            NodeOrToken::Token(child) => match child.kind() {
                SyntaxKind::Semicolon => vec![self.token(child)],
                SyntaxKind::Comment => self.comment(child, *newlines, at_start && prev.is_none()),
                kind => unreachable!("{kind:?} is not among statements"),
            },
        };
        separator.into_iter().chain(item).collect()
    }

    fn node(&mut self, node: &Node) -> Vec<Element<'a>> {
        match node {
            Node::Pipeline(pipeline) => self.pipeline(pipeline),
            Node::PipelineElement(_)
            | Node::Call(_)
            | Node::ExternalCall(_)
            | Node::MathExpression(_)
            | Node::Redirection(_)
            | Node::Match(_) => self.spaced(node.syntax()),
            Node::CellPath(_) | Node::Range(_) | Node::Spread(_) => self.touching(node.syntax()),
            Node::If(_) | Node::For(_) | Node::While(_) | Node::Loop(_) | Node::Try(_) => {
                self.bodies(node.syntax())
            },
            Node::MatchArms(arms) => self.container(arms),
            Node::MatchArm(arm) => self.arm(arm),
            Node::ListPattern(pattern) => self.container(pattern),
            Node::Def(def) => self.def(def),
            Node::List(list) => self.container(list),
            Node::Record(record) => self.container(record),
            Node::RecordField(field) => self.field(field),
            Node::Signature(signature) => self.container(signature),
            Node::Table(table) => self.table(table),
            Node::Param(param) => self.param(param),
            Node::Type(type_) => self.type_(type_),
            Node::Binary(binary) => self.binary(binary),
            Node::TypeField(field) => self.type_field(field),
            Node::Block(block) => self.block(block),
            Node::Subexpression(subexpression) => self.subexpression(subexpression),
            Node::Root(_) => unreachable!("Root is laid out by file"),
            Node::Assignment(_) | Node::Let(_) | Node::ClosureParams(_) => {
                unreachable!("{:?} is laid out by its parent", node.syntax().kind())
            },
            Node::Error(_) => unreachable!("a file with errors is not formatted"),
        }
    }

    /// Nu lexes a line break inside `( )` as whitespace, so one can fall among these children.
    fn spaced(&mut self, parent: &SyntaxNode) -> Vec<Element<'a>> {
        let items: Vec<_> = children(parent.children_with_tokens()).collect();
        self.spaced_children(&items, |_, item| Some(line_break_or_space(item)))
    }

    /// Children that touch one another, as the pieces of one word do.
    fn touching(&mut self, parent: &SyntaxNode) -> Vec<Element<'a>> {
        parent
            .children_with_tokens()
            .flat_map(|child| self.element(&ast::typed(child)))
            .collect()
    }

    /// `items` with what `plain` puts before each.
    /// A child `plain` gives nothing for touches what comes before it.
    fn spaced_children(
        &mut self,
        items: &[Child],
        plain: impl Fn(usize, &Child) -> Option<Element<'static>>,
    ) -> Vec<Element<'a>> {
        items
            .iter()
            .enumerate()
            .flat_map(|(index, item)| {
                let prev = items[..index].last().map(Child::kind);
                match item.as_comment() {
                    Some(comment) => self.comment(comment, item.newlines, false),
                    None => separator(prev, plain(index, item))
                        .into_iter()
                        .chain(self.element(&item.element))
                        .collect(),
                }
            })
            .collect()
    }

    fn element(&mut self, element: &ast::Element) -> Vec<Element<'a>> {
        match element {
            NodeOrToken::Node(node) => self.node(node),
            NodeOrToken::Token(token) => vec![self.token(token)],
        }
    }

    /// The one place a comment is printed, other than inside text kept as it is.
    fn comment(&self, comment: &SyntaxToken, newlines: u32, at_start: bool) -> Vec<Element<'a>> {
        if newlines > 0 || at_start {
            // The container may have pushed a line break already.
            // The printer keeps the longer of the two.
            vec![hard_line_break(), self.token(comment)]
        } else {
            // A line suffix, so that it does not count toward the width of the line.
            vec![line_suffix([space(), self.token(comment)])]
        }
    }

    fn token(&self, token: &SyntaxToken) -> Element<'a> {
        self.source(token.text_range())
    }

    fn source(&self, range: TextRange) -> Element<'a> {
        text(&self.src[range])
    }
}

/// Whether a `|` ending the last statement among `before` still reaches what comes next.
/// It does while no blank line has come since, which closes the pipe as it did in the source.
fn pipe_open(before: &[Child]) -> bool {
    before
        .iter()
        .rev()
        .find_map(|child| match &child.element {
            NodeOrToken::Node(statement) => Some(ends_with_pipe(statement.syntax())),
            NodeOrToken::Token(_) if child.newlines > 1 => Some(false),
            NodeOrToken::Token(_) => None,
        })
        .unwrap_or(false)
}

/// Whether the statement ends with a `|`.
/// Statements never end with trivia, so the last child tells.
fn ends_with_pipe(statement: &SyntaxNode) -> bool {
    statement
        .last_child_or_token()
        .is_some_and(|last| last.kind() == SyntaxKind::Pipe)
}

/// Whether the brackets of `node` stay as they are, with everything between them.
/// A comment inside ending in a lone `\r` keeps them so.
/// The line break printed after the comment would make that `\r` part of the break.
fn stays_as_is(node: &SyntaxNode) -> bool {
    node.children_with_tokens()
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

fn line_break_or_space(item: &Child) -> Element<'static> {
    if item.newlines > 0 {
        hard_line_break()
    } else {
        space()
    }
}

/// What separates the child after `prev` from it: `plain`, but a line break after a comment.
/// A comment runs to the end of its line.
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
