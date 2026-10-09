use nuten_doc::{
    Build, Element, empty_line, expand_parent, soft_line_break, soft_line_break_or_space, space,
    text,
};
use nuten_syntax::{
    NodeOrToken, SyntaxElement, SyntaxKind, SyntaxNode, SyntaxToken,
    ast::{self, AstNode, Bracketed, Node, RecordField, Table},
};

use super::{Child, Formatter, children, kept::Member, without_separators};
use crate::Commas;

type SoftLine = fn() -> Element<'static>;

#[derive(Clone, Copy)]
pub(super) struct ItemsLayout {
    pub(super) expand: bool,
    pub(super) before_first: SoftLine,
    pub(super) before_close: SoftLine,
    pub(super) commas: Commas,
}

impl Formatter<'_> {
    pub(super) fn container(&mut self, container: &dyn Bracketed) {
        let kind = container.syntax().kind();
        let line = if matches!(kind, SyntaxKind::Record | SyntaxKind::RecordPattern) {
            soft_line_break_or_space
        } else {
            soft_line_break
        };
        let is_arms = kind == SyntaxKind::MatchArms;
        // Arms are not the items of a value, which the setting is about.
        let commas = if is_arms { Commas::None } else { self.commas };
        self.items(
            |this| this.push(this.token(&container.open())),
            container.inside(),
            &container.close(),
            ItemsLayout {
                expand: is_arms,
                before_first: line,
                before_close: line,
                commas,
            },
        );
    }

    pub(super) fn table(&mut self, table: &Table) {
        let (columns, semicolon) = table.header();
        // As the head, a run kept from the columns through the `;` would join the `[`.
        // Formatted again, it would then take in the whole table.
        if self.kept.is_rest(&semicolon.clone().into()) {
            self.container(table);
            return;
        }
        let expand = children(table.inside(), &self.kept)
            .next()
            .is_some_and(|columns| columns.newlines > 0);
        self.items(
            |this| {
                this.push(this.token(&table.open()));
                this.element(&columns);
                this.push(this.token(&semicolon));
            },
            table.following(&semicolon.clone().into()),
            &table.close(),
            ItemsLayout {
                expand,
                before_first: soft_line_break_or_space,
                before_close: soft_line_break,
                commas: self.commas,
            },
        );
    }

    pub(super) fn items(
        &mut self,
        head: impl FnOnce(&mut Self),
        elements: impl Iterator<Item = SyntaxElement>,
        close: &SyntaxToken,
        layout: ItemsLayout,
    ) {
        let ItemsLayout {
            expand,
            before_first,
            before_close,
            commas,
        } = layout;
        let items: Vec<_> = without_separators(children(elements, &self.kept)).collect();
        let expand = expand
            || items.first().is_some_and(|first| first.newlines > 0)
            || items.iter().any(Child::is_comment);
        // A `,` beside a child that is no plain item would present the words around it as items.
        let commas = if items.iter().all(is_plain_item) {
            commas
        } else {
            Commas::None
        };
        if items.is_empty() {
            head(self);
            self.push(self.token(close));
            return;
        }
        self.group(|this| {
            head(this);
            this.indent(|this| {
                for (index, item) in items.iter().enumerate() {
                    this.item(index, item, before_first);
                    this.comma_after(&items, index, commas);
                }
            });
            this.extend([before_close(), this.token(close)]);
            if expand {
                this.push(expand_parent());
            }
        });
    }

    fn item(&mut self, index: usize, item: &Child, before_first: SoftLine) {
        let Child {
            element,
            newlines,
        } = item;
        let blank_line = index > 0 && *newlines > 1;
        match element {
            NodeOrToken::Token(comment) if comment.kind() == SyntaxKind::Comment => {
                if blank_line {
                    self.push(empty_line());
                }
                self.comment(comment, *newlines, false);
            },
            _ => {
                let separator = if index == 0 {
                    before_first()
                } else if blank_line {
                    empty_line()
                } else {
                    // After a comment, the container is expanded, so the line break comes anyway.
                    soft_line_break_or_space()
                };
                self.push(separator);
                self.element(element);
            },
        }
    }

    fn comma_after(&mut self, items: &[Child], index: usize, commas: Commas) {
        let item = &items[index];
        if item.is_comment() || self.keeps_comma(item) {
            return;
        }
        let is_last = items[index + 1..].iter().all(Child::is_comment);
        match (commas, is_last) {
            (Commas::Inline, false) => self.if_break(|_broken| {}, |flat| flat.push(text(","))),
            (Commas::Always | Commas::Trailing, false) => self.push(text(",")),
            (Commas::Trailing, true) => self.if_break(|broken| broken.push(text(",")), |_flat| {}),
            (Commas::None, _) | (Commas::Inline | Commas::Always, true) => {},
        }
    }

    /// Another `,` after an item kept with a `,` at its end would double that one.
    fn keeps_comma(&self, item: &Child) -> bool {
        matches!(
            self.kept.member(&ast::untyped(&item.element)),
            Some(Member::First(run)) if self.src[run].ends_with(',')
        )
    }

    pub(super) fn field(&mut self, field: &RecordField) {
        let items: Vec<_> =
            without_separators(children(field.syntax().children_with_tokens(), &self.kept))
                .collect();
        let colon = items
            .iter()
            .enumerate()
            .filter(|(_, item)| !item.is_comment())
            .nth(1)
            .filter(|(_, item)| item.kind() == SyntaxKind::Colon)
            .map(|(index, _)| index);
        self.spaced_children(&items, |index, _| (Some(index) != colon).then(space));
    }
}

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

fn parts(node: &SyntaxNode) -> Vec<SyntaxElement> {
    node.children_with_tokens()
        .filter(|part| !part.kind().is_trivia() && part.kind() != SyntaxKind::Comma)
        .collect()
}

fn is_operand(part: &SyntaxElement) -> bool {
    part.as_token().is_none_or(|token| token.kind().is_item())
}
