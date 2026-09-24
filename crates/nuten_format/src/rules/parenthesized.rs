//! Expressions nu reads inside `( )`, where its lexer takes a line break for a space.
//!
//! A call there stays on its line, though nu would take a line break before any argument.
//! The tree does not tell how many words its name takes, such as the two of `str replace`.

use std::iter;

use nuten_doc::{Element, breaks_groups, group, soft_line_break_or_space};
use nuten_parser::operator_precedence;
use nuten_syntax::{SyntaxKind, SyntaxNode};

use super::{Child, Formatter, children, line_break_or_space};

impl<'a> Formatter<'a> {
    /// An expression `breaks_by_width` picks, broken before each loosest operator if need be.
    /// Those split it into the parts nu evaluates last, so each line holds a whole part.
    pub(super) fn parenthesized_expression(&mut self, expression: &SyntaxNode) -> Vec<Element<'a>> {
        let items: Vec<_> = children(expression.children_with_tokens(), &self.kept).collect();
        let laid_out = self
            .laid_out_ahead
            .remove(expression)
            .unwrap_or_else(|| self.laid_out_items(&items));
        let broken_at = broken_at(&items, &laid_out);
        // An operator the source ends a line with, or puts a comment after, stays there.
        // Moving it would override where the author broke the line.
        let breaks_after = |index: usize| {
            items
                .get(index + 1)
                .is_some_and(|next| next.newlines > 0 || next.is_comment())
        };
        let mut laid_out = laid_out.into_iter();
        let expression = self.spaced_children_by(
            &items,
            |index, item| {
                let breaks_before = item.newlines == 0
                    && !breaks_after(index)
                    && precedence_of(item).is_some_and(|precedence| Some(precedence) == broken_at);
                Some(if breaks_before {
                    soft_line_break_or_space()
                } else {
                    line_break_or_space(item)
                })
            },
            |_, _| {
                laid_out
                    .next()
                    .expect("`spaced_children_by` lays out each item but a comment, in order")
            },
        );
        vec![group(expression)]
    }

    /// Whether `element`, an element of a pipeline, holds an expression that breaks at operators.
    /// Its items stay laid out for `parenthesized_expression` to take.
    /// False when the expression or what holds it is kept, since nothing would take its items.
    pub(super) fn breaks_at_operators(&mut self, element: &SyntaxNode) -> bool {
        let Some(expression) = held_expression(element).filter(|expression| {
            breaks_by_width(expression) && !self.kept_up_to(expression, element)
        }) else {
            return false;
        };
        let items: Vec<_> = children(expression.children_with_tokens(), &self.kept).collect();
        let laid_out = self.laid_out_items(&items);
        let breaks = broken_at(&items, &laid_out).is_some();
        self.laid_out_ahead.insert(expression, laid_out);
        breaks
    }

    /// Whether `expression`, or what holds it up to `element`, is kept as written.
    fn kept_up_to(&self, expression: &SyntaxNode, element: &SyntaxNode) -> bool {
        expression
            .ancestors()
            .take_while(|node| node != element)
            .chain(iter::once(element.clone()))
            .any(|node| self.kept.keeps(&node.into()))
    }

    /// Each of `items` but a comment as the formatter lays it out.
    fn laid_out_items(&mut self, items: &[Child]) -> Vec<Vec<Element<'a>>> {
        items
            .iter()
            .filter(|item| !item.is_comment())
            .map(|item| self.element(&item.element))
            .collect()
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

/// The precedence of the operators an expression breaks before: its loosest.
/// `None` where it breaks before none, as where one of `laid_out` is laid out over lines.
/// Such an item breaks the group around it whatever the width.
/// The expression would then break at its operators even where it fits.
/// The layout decides, not the source, which may have line breaks the layout drops.
fn broken_at(items: &[Child], laid_out: &[Vec<Element<'_>>]) -> Option<u8> {
    if laid_out.iter().any(|item| breaks_groups(item)) {
        None
    } else {
        items.iter().filter_map(precedence_of).min()
    }
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
            && holder
                .parent()
                .is_none_or(|parent| parent.kind() != SyntaxKind::Interpolation)
            && !holder
                .children_with_tokens()
                .any(|child| child.kind() == SyntaxKind::Semicolon)
    })
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
