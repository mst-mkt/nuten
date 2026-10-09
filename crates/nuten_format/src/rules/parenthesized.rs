//! Expressions nu reads inside `( )`, where its lexer takes a line break for a space.

use nuten_doc::{Build, soft_line_break_or_space, space};
use nuten_parser::operator_precedence;
use nuten_syntax::{SyntaxKind, SyntaxNode};

use super::{Child, Formatter, breaks_by_width, children, line_break_or_space, starts_kept_line};

impl Formatter<'_> {
    /// Each line then holds a whole part, since nu evaluates the loosest operators last.
    pub(super) fn parenthesized_expression(&mut self, expression: &SyntaxNode) {
        let items: Vec<_> = children(expression.children_with_tokens(), &self.kept).collect();
        let keeps_line_breaks = (0..items.len()).any(|index| {
            items[index].is_comment() || starts_kept_line(&self.kept, &items[index..])
        });
        // The levels of `operation` would ignore the line breaks a comment or a kept line keeps.
        if !keeps_line_breaks {
            self.operation(&items, 0);
            return;
        }
        let loosest = loosest_precedence(&items);
        // An operator ahead of a comment or a kept line stays, or it would sit alone on its line.
        let breaks_after: Vec<_> = (0..items.len())
            .map(|index| {
                items.get(index + 1).is_some_and(|next| {
                    next.is_comment() || starts_kept_line(&self.kept, &items[index + 1..])
                })
            })
            .collect();
        self.group(|this| {
            this.spaced_children(&items, |index, item| {
                let breaks_before = !breaks_after[index]
                    && precedence_of(item).is_some_and(|precedence| Some(precedence) == loosest);
                Some(if breaks_before {
                    soft_line_break_or_space()
                } else {
                    line_break_or_space(item)
                })
            });
        });
    }

    fn operation(&mut self, items: &[Child], levels: u32) {
        let loosest = loosest_precedence(items);
        let operators: Vec<_> = (0..items.len())
            .filter(|&index| {
                precedence_of(&items[index]).is_some_and(|precedence| Some(precedence) == loosest)
            })
            .collect();
        // An operator first leaves no part before it to break after.
        let Some(&first) = operators.first().filter(|&&first| first > 0) else {
            self.spaced_children(items, |_, _| Some(space()));
            return;
        };
        let ends = operators.iter().skip(1).copied().chain([items.len()]);
        self.group(|this| {
            // A first part goes deeper, or its breaks would line up with the operators after it.
            this.operation(&items[..first], levels + 1);
            this.indent_levels(levels, |this| {
                for (&index, end) in operators.iter().zip(ends) {
                    this.push(soft_line_break_or_space());
                    this.element(&items[index].element);
                    this.push(space());
                    this.operation(&items[index + 1..end], 1);
                }
            });
        });
    }

    pub(super) fn holds_expression_breaking_at_operators(&self, element: &SyntaxNode) -> bool {
        held_expression(element).is_some_and(|expression| {
            breaks_by_width(&expression)
                && !self.kept_up_to(&expression, element)
                && loosest_precedence(
                    &children(expression.children_with_tokens(), &self.kept).collect::<Vec<_>>(),
                )
                .is_some()
        })
    }
}

fn held_expression(element: &SyntaxNode) -> Option<SyntaxNode> {
    element.first_child().and_then(|node| match node.kind() {
        SyntaxKind::MathExpression => Some(node),
        SyntaxKind::Where => node
            .children()
            .find(|child| child.kind() == SyntaxKind::MathExpression),
        _ => None,
    })
}

fn loosest_precedence(items: &[Child]) -> Option<u8> {
    items.iter().filter_map(precedence_of).min()
}

/// `not` is no operator here, since it takes the term after it.
/// Nor is a word spelled as one where nu reads a term, since that is no `Operator`.
fn precedence_of(item: &Child) -> Option<u8> {
    item.element
        .as_token()
        .filter(|token| token.kind() == SyntaxKind::Operator)
        .and_then(|token| operator_precedence(token.text()))
}
