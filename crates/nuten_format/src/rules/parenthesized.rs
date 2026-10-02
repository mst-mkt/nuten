//! Expressions nu reads inside `( )`, where its lexer takes a line break for a space.
//!
//! A call there stays on its line, though nu would take a line break before any argument.

use std::iter;

use nuten_doc::{Element, group, indent, soft_line_break_or_space, space};
use nuten_parser::operator_precedence;
use nuten_syntax::{SyntaxKind, SyntaxNode};

use super::{Child, Formatter, children, in_interpolation, line_break_or_space, starts_kept_line};

impl<'a> Formatter<'a> {
    /// An expression `breaks_by_width` picks, broken before its loosest operators if need be.
    /// Those split it into the parts nu evaluates last, so each line holds a whole part.
    pub(super) fn parenthesized_expression(&mut self, expression: &SyntaxNode) -> Vec<Element<'a>> {
        let items: Vec<_> = children(expression.children_with_tokens(), &self.kept).collect();
        let keeps_line_breaks = (0..items.len()).any(|index| {
            items[index].is_comment() || starts_kept_line(&self.kept, &items[index..])
        });
        // The levels of `operation` would ignore the line breaks a comment or a kept line keeps.
        if !keeps_line_breaks {
            return self.operation(&items, 0);
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
        let expression = self.spaced_children(&items, |index, item| {
            let breaks_before = !breaks_after[index]
                && precedence_of(item).is_some_and(|precedence| Some(precedence) == loosest);
            Some(if breaks_before {
                soft_line_break_or_space()
            } else {
                line_break_or_space(item)
            })
        });
        vec![group(expression)]
    }

    /// `items` broken before each loosest operator where they do not fit, and each part likewise.
    fn operation(&mut self, items: &[Child], levels: usize) -> Vec<Element<'a>> {
        let loosest = loosest_precedence(items);
        let operators: Vec<_> = (0..items.len())
            .filter(|&index| {
                precedence_of(&items[index]).is_some_and(|precedence| Some(precedence) == loosest)
            })
            .collect();
        // An operator first leaves no part before it to break after.
        let Some(&first) = operators.first().filter(|&&first| first > 0) else {
            return self.spaced_children(items, |_, _| Some(space()));
        };
        // A first part goes deeper, or its own breaks would line up with the operators after it.
        let head = self.operation(&items[..first], levels + 1);
        let ends = operators.iter().skip(1).copied().chain([items.len()]);
        let rest: Vec<_> = operators
            .iter()
            .zip(ends)
            .flat_map(|(&index, end)| {
                iter::once(soft_line_break_or_space())
                    .chain(self.element(&items[index].element))
                    .chain([space()])
                    .chain(self.operation(&items[index + 1..end], 1))
                    .collect::<Vec<_>>()
            })
            .collect();
        let rest = (0..levels).fold(rest, |rest, _| vec![indent(rest)]);
        vec![group(head.into_iter().chain(rest))]
    }

    /// Whether `element`, an element of a pipeline, holds an expression that breaks at operators.
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

    fn kept_up_to(&self, expression: &SyntaxNode, element: &SyntaxNode) -> bool {
        expression
            .ancestors()
            .take_while(|node| node != element)
            .chain(iter::once(element.clone()))
            .any(|node| self.kept.keeps(&node.into()))
    }
}

/// The expression `element` holds: its first child, or the row condition of its `where`.
fn held_expression(element: &SyntaxNode) -> Option<SyntaxNode> {
    element.first_child().and_then(|node| match node.kind() {
        SyntaxKind::MathExpression => Some(node),
        SyntaxKind::Where => node
            .children()
            .find(|child| child.kind() == SyntaxKind::MathExpression),
        _ => None,
    })
}

/// Whether `expression` is laid out by `parenthesized_expression`.
/// It is what the only statement right inside `( )` holds.
/// It reaches that `( )` through its pipeline, its element, and any `where`, but nothing else.
/// A `( )` inside a string is left out, since a line break there also splits the string.
/// So is what an assignment takes, whose holder is no `Subexpression`.
/// Nu ends it at a line break.
/// So is a statement that shares its `( )` with another.
/// Its `;` would then end a broken line, as if the next statement went on from it.
pub(super) fn breaks_by_width(expression: &SyntaxNode) -> bool {
    let holder = expression.ancestors().skip(1).find(|ancestor| {
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

fn loosest_precedence(items: &[Child]) -> Option<u8> {
    items.iter().filter_map(precedence_of).min()
}

/// How tightly `item` binds as an operator, the smaller the looser.
/// `None` for anything else, `not` among them, which takes the term after it.
/// `None` too for a word spelled as one where nu reads a term, since that is no `Operator`.
fn precedence_of(item: &Child) -> Option<u8> {
    item.element
        .as_token()
        .filter(|token| token.kind() == SyntaxKind::Operator)
        .and_then(|token| operator_precedence(token.text()))
}
