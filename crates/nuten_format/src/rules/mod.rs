mod alias;
mod attribute;
mod binary;
mod block;
mod call;
mod container;
mod control;
mod def;
mod interpolation;
mod kept;
mod match_;
mod module;
mod parenthesized;
mod pipeline;
mod signature;
mod subexpression;

use std::{iter, mem};

use nuten_doc::{
    Element, GroupIds, empty_line, empty_lines, exact_text, hard_line_break, line_suffix, space,
    text,
};
use nuten_syntax::{
    NodeOrToken, SyntaxElement, SyntaxKind, SyntaxNode, SyntaxToken, TextRange, WalkEvent,
    ast::{self, AstNode, Node},
};

use self::kept::{Kept, Member};
use crate::Commas;

struct Child {
    element: ast::Element,
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

fn children(
    elements: impl IntoIterator<Item = SyntaxElement>,
    kept: &Kept,
) -> impl Iterator<Item = Child> {
    let mut newlines = 0;
    elements
        .into_iter()
        .filter_map(move |element| match element.kind() {
            SyntaxKind::Whitespace => None,
            _ if kept.is_rest(&element) => None,
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

fn without_separators(children: impl Iterator<Item = Child>) -> impl Iterator<Item = Child> {
    let mut dropped = 0;
    children.filter_map(move |child| {
        let newlines = child.newlines.max(mem::take(&mut dropped));
        if child.kind().is_separator() {
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
pub(crate) fn file<'a>(src: &'a str, root: &SyntaxNode, commas: Commas) -> Vec<Element<'a>> {
    let mut formatter = Formatter {
        src,
        root: root.clone(),
        groups: GroupIds::default(),
        kept: Kept::of(src, root),
        commas,
    };
    let statements: Vec<_> = children(root.children_with_tokens(), &formatter.kept).collect();
    let mut out = formatter.statements(&statements, true);
    out.push(hard_line_break());
    out
}

impl Commas {
    fn on_one_line(self) -> bool {
        self != Commas::None
    }
}

struct Formatter<'a> {
    src: &'a str,
    /// The tree of `src`.
    root: SyntaxNode,
    groups: GroupIds,
    kept: Kept,
    commas: Commas,
}

impl<'a> Formatter<'a> {
    /// Line breaks before the first statement are left to the container.
    fn statements(&mut self, items: &[Child], at_start: bool) -> Vec<Element<'a>> {
        items
            .iter()
            .enumerate()
            .flat_map(|(index, item)| self.statement(&items[..index], item, at_start))
            .collect()
    }

    /// `before` is every child that comes before it.
    fn statement(
        &mut self,
        before: &[Child],
        item: &Child,
        at_start: bool,
    ) -> impl Iterator<Item = Element<'a>> + use<'a> {
        let Child {
            element,
            newlines,
        } = item;
        let prev = before.last().map(Child::kind);
        let separator = match (prev, *newlines) {
            (Some(SyntaxKind::Semicolon), 0) if element.as_node().is_some() => Some(space()),
            (None, _) | (_, 0) => None,
            (_, 1) => Some(hard_line_break()),
            // With one blank line, a `|` starting the next line would join the statement above.
            // So two stay.
            (_, 3..) if reaches_past_blank_line(&ast::untyped(element)) => Some(empty_lines(2)),
            _ => Some(empty_line()),
        };
        let elements = match element {
            NodeOrToken::Node(_) => self.element(element),
            NodeOrToken::Token(child) => match child.kind() {
                SyntaxKind::Semicolon => vec![self.token(child)],
                SyntaxKind::Comment => self.comment(child, *newlines, at_start && prev.is_none()),
                kind => unreachable!("{kind:?} is not among statements"),
            },
        };
        separator.into_iter().chain(elements)
    }

    fn node(&mut self, node: &Node) -> Vec<Element<'a>> {
        match node {
            Node::Pipeline(pipeline) => self.pipeline(pipeline),
            Node::MathExpression(_) if breaks_by_width(node.syntax()) => {
                self.parenthesized_expression(node.syntax())
            },
            Node::Call(_) | Node::ExternalCall(_) => self.call(node.syntax()),
            Node::PipelineElement(_)
            | Node::CommandName(_)
            | Node::MathExpression(_)
            | Node::Redirection(_)
            | Node::Match(_)
            | Node::Use(_)
            | Node::Where(_)
            | Node::AttributeBlock(_) => self.spaced(node.syntax()),
            Node::Attribute(attribute) => self.attribute(attribute),
            Node::CellPath(_) | Node::Range(_) | Node::Spread(_) => self.touching(node.syntax()),
            Node::Interpolation(interpolation) => self.interpolation(interpolation),
            Node::If(_) | Node::For(_) | Node::While(_) | Node::Loop(_) | Node::Try(_) => {
                self.bodies(node.syntax())
            },
            Node::MatchArms(arms) => self.container(arms),
            Node::MatchArm(arm) => self.arm(arm),
            Node::ListPattern(pattern) => self.container(pattern),
            Node::RecordPattern(pattern) => self.container(pattern),
            Node::Def(def) => self.def(def),
            Node::Alias(alias) => self.alias(alias),
            Node::Module(module) => self.module(module),
            Node::ExportEnv(export_env) => self.export_env(export_env),
            Node::List(list) => self.container(list),
            Node::Record(record) => self.container(record),
            Node::RecordField(field) => self.field(field),
            Node::Signature(signature) => self.container(signature),
            Node::Table(table) => self.table(table),
            Node::Param(param) => self.param(param),
            Node::Type(type_) => self.type_(type_),
            Node::Binary(binary) => self.binary(binary),
            Node::TypeField(field) => self.type_field(field),
            Node::IoTypes(types) => self.io_types(types),
            Node::IoType(pair) => self.io_type(pair),
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
        let items: Vec<_> = children(parent.children_with_tokens(), &self.kept).collect();
        self.spaced_children(&items, |_, item| {
            (item.kind() != SyntaxKind::Semicolon).then(|| line_break_or_space(item))
        })
    }

    fn touching(&mut self, parent: &SyntaxNode) -> Vec<Element<'a>> {
        parent
            .children_with_tokens()
            .flat_map(|child| self.element(&ast::typed(child)))
            .collect()
    }

    fn spaced_children(
        &mut self,
        items: &[Child],
        plain: impl Fn(usize, &Child) -> Option<Element<'static>>,
    ) -> Vec<Element<'a>> {
        self.spaced_children_by(items, plain, |this, _, element| this.element(element))
    }

    fn spaced_children_by(
        &mut self,
        items: &[Child],
        plain: impl Fn(usize, &Child) -> Option<Element<'static>>,
        mut element: impl FnMut(&mut Self, usize, &ast::Element) -> Vec<Element<'a>>,
    ) -> Vec<Element<'a>> {
        items
            .iter()
            .enumerate()
            .flat_map(|(index, item)| {
                let prev = items[..index].last().map(Child::kind);
                let (lead, elements) = match item.as_comment() {
                    Some(comment) => (None, self.comment(comment, item.newlines, false)),
                    None if starts_kept_line(&self.kept, &items[index..]) => {
                        (Some(hard_line_break()), element(self, index, &item.element))
                    },
                    None => (
                        separator(prev, plain(index, item)),
                        element(self, index, &item.element),
                    ),
                };
                lead.into_iter().chain(elements)
            })
            .collect()
    }

    fn kept_up_to(&self, node: &SyntaxNode, element: &SyntaxNode) -> bool {
        node.ancestors()
            .take_while(|node| node != element)
            .chain(iter::once(element.clone()))
            .any(|node| self.kept.keeps(&node.into()))
    }

    fn one_line_items(
        &mut self,
        elements: impl Iterator<Item = SyntaxElement>,
        comma_before: impl Fn(&Child) -> bool,
    ) -> Vec<Element<'a>> {
        let items: Vec<_> = without_separators(children(elements, &self.kept)).collect();
        items
            .iter()
            .enumerate()
            .flat_map(|(index, item)| {
                let separator = match index {
                    0 => Vec::new(),
                    _ if comma_before(item) => vec![text(","), space()],
                    _ => vec![space()],
                };
                separator.into_iter().chain(self.element(&item.element))
            })
            .collect()
    }

    fn element(&mut self, element: &ast::Element) -> Vec<Element<'a>> {
        match (self.kept.member(&ast::untyped(element)), element) {
            (Some(Member::First(run)), _) => self.source(run),
            (Some(Member::Rest), _) => Vec::new(),
            (None, NodeOrToken::Node(node)) => self.node(node),
            (None, NodeOrToken::Token(token)) => vec![self.token(token)],
        }
    }

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
        let source = &self.src[token.text_range()];
        if spans_lines(token) {
            exact_text(source)
        } else {
            text(source)
        }
    }

    fn source(&self, range: TextRange) -> Vec<Element<'a>> {
        let exact = exact_ranges(&self.root, range);
        let gaps = iter::once(range.start())
            .chain(exact.iter().map(|exact| exact.end()))
            .zip(
                exact
                    .iter()
                    .map(|exact| exact.start())
                    .chain(iter::once(range.end())),
            )
            .map(|(start, end)| TextRange::new(start, end));
        let exact_texts = exact
            .iter()
            .map(|&exact| Some(exact_text(&self.src[exact])))
            .chain(iter::once(None));
        gaps.zip(exact_texts)
            .flat_map(|(gap, exact_text)| {
                let gap = (!gap.is_empty()).then(|| text(&self.src[gap]));
                gap.into_iter().chain(exact_text)
            })
            .collect()
    }
}

/// Nu reads the line breaks inside such a token as part of its value, byte for byte.
/// Only a `Newline` holds one that is layout.
fn spans_lines(token: &SyntaxToken) -> bool {
    token.kind() != SyntaxKind::Newline && token.text().contains('\n')
}

/// A line break inside an attribute block is not layout either, since nu takes it into the code.
pub(crate) fn is_layout_line_break(token: &SyntaxToken) -> bool {
    token.kind() == SyntaxKind::Newline
        && !token
            .parent_ancestors()
            .any(|node| is_attribute_block(&node.into()))
}

/// `range` holds whole elements.
fn exact_ranges(root: &SyntaxNode, range: TextRange) -> Vec<TextRange> {
    let within = match root.covering_element(range) {
        NodeOrToken::Node(node) => node,
        NodeOrToken::Token(token) => token.parent().expect("a token has a parent"),
    };
    let mut walk = within.preorder_with_tokens();
    iter::from_fn(|| {
        while let Some(event) = walk.next() {
            let WalkEvent::Enter(element) = event else {
                continue;
            };
            let element_range = element.text_range();
            match &element {
                // Only the covering node reaches past `range`, so walk into it.
                _ if element_range.contains_range(range) && element_range != range => {},
                _ if !range.contains_range(element_range) => walk.skip_subtree(),
                NodeOrToken::Node(_) if is_attribute_block(&element) => {
                    walk.skip_subtree();
                    return Some(element_range);
                },
                NodeOrToken::Token(token) if spans_lines(token) => return Some(element_range),
                _ => {},
            }
        }
        None
    })
    .collect()
}

/// Nu takes the text of an `@example` block, line breaks and all, into the code of the example.
/// The blocks of other attributes count too, since `attribute` prints every one as written.
fn is_attribute_block(element: &SyntaxElement) -> bool {
    element.kind() == SyntaxKind::Block
        && element
            .parent()
            .is_some_and(|parent| parent.kind() == SyntaxKind::Attribute)
}

/// Nu goes on past one blank line after a call that an attribute heads.
/// Siblings tell rather than children, since a run kept as written leaves only its first child.
fn reaches_past_blank_line(item: &SyntaxElement) -> bool {
    siblings_before(item)
        .filter(|sibling| !matches!(sibling.kind(), SyntaxKind::Whitespace | SyntaxKind::Newline))
        .find_map(|sibling| match &sibling {
            // A closure's parameters are a sibling before its first statement, not a statement.
            NodeOrToken::Node(statement) => Some(
                statement.kind() == SyntaxKind::Pipeline
                    && (ends_with_pipe(statement) || ends_with_attributed_call(statement)),
            ),
            NodeOrToken::Token(_) if newlines_before(&sibling) > 1 => Some(false),
            NodeOrToken::Token(_) => None,
        })
        .unwrap_or(false)
}

fn siblings_before(element: &SyntaxElement) -> impl Iterator<Item = SyntaxElement> {
    iter::successors(
        element.prev_sibling_or_token(),
        SyntaxElement::prev_sibling_or_token,
    )
}

fn newlines_before(element: &SyntaxElement) -> usize {
    siblings_before(element)
        .filter(|sibling| sibling.kind() != SyntaxKind::Whitespace)
        .take_while(|sibling| sibling.kind() == SyntaxKind::Newline)
        .count()
}

/// Statements never end with trivia, so the last token tells.
fn ends_with_pipe(statement: &SyntaxNode) -> bool {
    statement
        .last_token()
        .is_some_and(|last| last.kind() == SyntaxKind::Pipe)
}

/// The call may end with a command instead, which a blank line closes.
/// Two blank lines then keep a wider gap than needed, but close it as well.
fn ends_with_attributed_call(statement: &SyntaxNode) -> bool {
    statement
        .children()
        .last()
        .and_then(|element| element.first_child())
        .is_some_and(|call| call.kind() == SyntaxKind::Call && headed_by_attribute(&call))
}

/// The head tells, since nu reads the attribute and what follows it as one call.
fn headed_by_attribute(node: &SyntaxNode) -> bool {
    node.first_token()
        .is_some_and(|head| head.text().starts_with('@'))
}

fn line_break_or_space(item: &Child) -> Element<'static> {
    if item.newlines > 0 && !joins_line_break(&ast::untyped(&item.element)) {
        hard_line_break()
    } else {
        space()
    }
}

/// The nearest brackets around it decide, since nu lexes what each holds by its own rules.
/// A call an attribute heads stops the search too, since a line break ends the attribute.
/// One inside a string is left as it is, since a line break there also splits the string.
fn joins_line_break(element: &SyntaxElement) -> bool {
    let Some(parent) = element.parent() else {
        return false;
    };
    let nearest = parent.ancestors().find(|ancestor| {
        matches!(
            ancestor.kind(),
            SyntaxKind::Subexpression
                | SyntaxKind::Block
                | SyntaxKind::MatchArms
                | SyntaxKind::Record
                | SyntaxKind::List
                | SyntaxKind::Table
                | SyntaxKind::Signature
                | SyntaxKind::AttributeBlock
                | SyntaxKind::Attribute
        ) || headed_by_attribute(ancestor)
    });
    nearest.is_some_and(|nearest| nearest.kind() == SyntaxKind::Subexpression)
        && !in_interpolation(&parent)
}

/// A `( )` inside a string is left out, since a line break there also splits the string.
/// So is what an assignment takes, since nu ends it at a line break.
/// So is a statement that shares its `( )` with another.
/// Its `;` would then end a broken line, as if the next statement went on from it.
fn breaks_by_width(node: &SyntaxNode) -> bool {
    let holder = node.ancestors().skip(1).find(|ancestor| {
        !matches!(
            ancestor.kind(),
            SyntaxKind::PipelineElement | SyntaxKind::Pipeline | SyntaxKind::Where
        )
    });
    holder.is_some_and(|holder| {
        holder.kind() == SyntaxKind::Subexpression
            && !in_interpolation(&holder)
            && !holder
                .children_with_tokens()
                .any(|child| child.kind() == SyntaxKind::Semicolon)
    })
}

fn in_interpolation(node: &SyntaxNode) -> bool {
    node.ancestors()
        .any(|ancestor| ancestor.kind() == SyntaxKind::Interpolation)
}

/// The layout would otherwise join a line that starts with something kept to the one above.
/// Formatted again, the kept run on it would then take in that line too.
fn starts_kept_line<'c>(kept: &Kept, items: impl IntoIterator<Item = &'c Child>) -> bool {
    let mut items = items.into_iter();
    items.next().is_some_and(|first| {
        first.newlines > 0
            && iter::once(first)
                .chain(items.take_while(|next| next.newlines == 0))
                .any(|item| kept.keeps(&ast::untyped(&item.element)))
    })
}

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
