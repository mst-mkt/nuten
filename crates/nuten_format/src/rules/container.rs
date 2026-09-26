use std::iter;

use nuten_doc::{
    Element, empty_line, expand_parent, group, if_break, indent, soft_line_break,
    soft_line_break_or_space, space, text,
};
use nuten_syntax::{
    NodeOrToken, SyntaxElement, SyntaxKind, SyntaxNode, SyntaxToken,
    ast::{self, AstNode, Bracketed, Node, RecordField, Table},
};

use super::{Child, Formatter, children, kept::Member, without_commas};
use crate::Commas;

/// Makes a line break that prints as nothing or as a space when the group is flat.
type SoftLine = fn() -> Element<'static>;

impl<'a> Formatter<'a> {
    pub(super) fn container(&mut self, container: &dyn Bracketed) -> Vec<Element<'a>> {
        let kind = container.syntax().kind();
        let line = if matches!(kind, SyntaxKind::Record | SyntaxKind::RecordPattern) {
            soft_line_break_or_space
        } else {
            soft_line_break
        };
        let head = vec![self.token(&container.open())];
        let is_arms = kind == SyntaxKind::MatchArms;
        // Arms are not the items of a value, which the setting is about.
        let commas = if is_arms { Commas::None } else { self.commas };
        self.items(
            head,
            container.inside(),
            is_arms,
            line,
            line,
            &container.close(),
            commas,
        )
    }

    pub(super) fn table(&mut self, table: &Table) -> Vec<Element<'a>> {
        let (columns, semicolon) = table.header();
        // As the head, a run kept from the columns through the `;` would join the `[`.
        // Formatted again, it would then take in the whole table.
        if self.kept.is_rest(&semicolon.clone().into()) {
            return self.container(table);
        }
        let expand = children(table.inside(), &self.kept)
            .next()
            .is_some_and(|columns| columns.newlines > 0);
        let mut head = vec![self.token(&table.open())];
        head.extend(self.element(&columns));
        head.push(self.token(&semicolon));
        self.items(
            head,
            table.following(&semicolon.into()),
            expand,
            soft_line_break_or_space,
            soft_line_break,
            &table.close(),
            self.commas,
        )
    }

    #[expect(
        clippy::too_many_arguments,
        reason = "each container lays out its items with its own choice of these"
    )]
    pub(super) fn items(
        &mut self,
        head: Vec<Element<'a>>,
        elements: impl Iterator<Item = SyntaxElement>,
        expand: bool,
        before_first: SoftLine,
        before_close: SoftLine,
        close: &SyntaxToken,
        commas: Commas,
    ) -> Vec<Element<'a>> {
        let items: Vec<_> = without_commas(children(elements, &self.kept)).collect();
        let expand = expand
            || items.first().is_some_and(|first| first.newlines > 0)
            || items.iter().any(Child::is_comment);
        // A `,` beside a child that is no plain item would present the words around it as items.
        let commas = if items.iter().all(is_plain_item) {
            commas
        } else {
            Commas::None
        };
        let inner: Vec<_> = items
            .iter()
            .enumerate()
            .flat_map(|(index, item)| {
                let comma = self.comma_after(&items, index, commas);
                self.item(index, item, before_first)
                    .into_iter()
                    .chain(comma)
            })
            .collect();
        let mut out = head;
        if inner.is_empty() {
            out.push(self.token(close));
            return out;
        }
        out.push(indent(inner));
        out.push(before_close());
        out.push(self.token(close));
        if expand {
            out.push(expand_parent());
        }
        vec![group(out)]
    }

    fn item(&mut self, index: usize, item: &Child, before_first: SoftLine) -> Vec<Element<'a>> {
        let Child {
            element,
            newlines,
        } = item;
        let blank_line = index > 0 && *newlines > 1;
        match element {
            NodeOrToken::Token(comment) if comment.kind() == SyntaxKind::Comment => blank_line
                .then(empty_line)
                .into_iter()
                .chain(self.comment(comment, *newlines, false))
                .collect(),
            _ => {
                let separator = if index == 0 {
                    before_first()
                } else if blank_line {
                    empty_line()
                } else {
                    // After a comment, the container is expanded, so the line break comes anyway.
                    soft_line_break_or_space()
                };
                iter::once(separator).chain(self.element(element)).collect()
            },
        }
    }

    /// The `,` between `items[index]` and the next item, comments aside, or after the last item.
    fn comma_after(&self, items: &[Child], index: usize, commas: Commas) -> Option<Element<'a>> {
        let item = &items[index];
        if item.is_comment() || self.keeps_comma(item) {
            None
        } else if items[index + 1..].iter().any(|next| !next.is_comment()) {
            commas.between()
        } else {
            commas.after_last()
        }
    }

    /// Whether `item` is kept as written with a `,` at its end, as a parameter holds its own.
    /// Another `,` after it would double that one.
    fn keeps_comma(&self, item: &Child) -> bool {
        matches!(
            self.kept.member(&ast::untyped(&item.element)),
            Some(Member::First(run)) if self.src[run].ends_with(',')
        )
    }

    pub(super) fn field(&mut self, field: &RecordField) -> Vec<Element<'a>> {
        let items: Vec<_> =
            without_commas(children(field.syntax().children_with_tokens(), &self.kept)).collect();
        let colon = items
            .iter()
            .enumerate()
            .filter(|(_, item)| !item.is_comment())
            .nth(1)
            .filter(|(_, item)| item.kind() == SyntaxKind::Colon)
            .map(|(index, _)| index);
        self.spaced_children(&items, |index, _| (Some(index) != colon).then(space))
    }
}

/// Whether nu reads `item` as the one item it looks like, or it is a comment.
/// Nu reads no item in a `|` or a redirection, nor one in a field without both key and value.
/// Nor in a parameter that ends with a `:` or a `=`, where nu would take a `,` for what it lacks.
fn is_plain_item(item: &Child) -> bool {
    match &item.element {
        NodeOrToken::Token(token) => token.kind() == SyntaxKind::Comment || token.kind().is_item(),
        NodeOrToken::Node(Node::RecordField(field)) => matches!(
            parts(field.syntax()).as_slice(),
            [key, colon, value]
                if colon.kind() == SyntaxKind::Colon && is_operand(key) && is_operand(value)
        ),
        NodeOrToken::Node(Node::Param(param)) => !parts(param.syntax())
            .last()
            .is_some_and(|last| matches!(last.kind(), SyntaxKind::Colon | SyntaxKind::Eq)),
        NodeOrToken::Node(_) => true,
    }
}

/// The children of `node` but trivia and `,`s.
fn parts(node: &SyntaxNode) -> Vec<SyntaxElement> {
    node.children_with_tokens()
        .filter(|part| !part.kind().is_trivia() && part.kind() != SyntaxKind::Comma)
        .collect()
}

fn is_operand(part: &SyntaxElement) -> bool {
    part.as_token().is_none_or(|token| token.kind().is_item())
}

impl Commas {
    fn between(self) -> Option<Element<'static>> {
        match self {
            Commas::None => None,
            Commas::Inline => Some(if_break([], [text(",")])),
            Commas::Always | Commas::Trailing => Some(text(",")),
        }
    }

    fn after_last(self) -> Option<Element<'static>> {
        (self == Commas::Trailing).then(|| if_break([text(",")], []))
    }
}
