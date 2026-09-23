use nuten_syntax::SyntaxKind;

use super::{
    assignment::Assigned,
    command::{argument_shape, is_math_expression_like, start_call},
    ends_call, ends_statement, item,
    keyword::keyword,
    keyword_words,
    pipeline::{bump_newlines, next_line_starts_with_pipe, rest_of_statement},
};
use crate::parser::Parser;

/// Whether the element ahead is an alias nu declares: `alias`, a name, `=`, and what follows.
/// Nu declares none where a value heads what follows `=`, and rejects the statement instead.
pub(super) fn alias_ahead(p: &Parser<'_>) -> bool {
    let name = keyword_words(p);
    p.nth_text(name - 1) == Some("alias")
        && p.nth(name).is_some_and(SyntaxKind::is_item)
        && p.nth(name + 1) == Some(SyntaxKind::Assign)
        && p.nth_text(name + 1) == Some("=")
        && match p.nth(name + 2) {
            Some(SyntaxKind::Newline) => next_line_starts_with_pipe(p),
            kind => kind.is_some_and(|kind| !ends_statement(kind)),
        }
        && p.nth_text(name + 2)
            .is_some_and(|head| !is_math_expression_like(head))
}

/// The `Alias` ahead: what `alias_ahead` takes up to `=`, then what follows.
/// `assigned` tells where the statement ends, as for a pipeline.
pub(super) fn alias(p: &mut Parser<'_>, assigned: Assigned) {
    p.start(SyntaxKind::Alias);
    for _ in 0..keyword_words(p) + 2 {
        p.bump();
    }
    replacement(p, assigned);
    p.finish();
}

/// What the alias stands for, as nu's `parse_call` reads the rest of the statement.
fn replacement(p: &mut Parser<'_>, assigned: Assigned) {
    // Nu's lexer joins a line that starts with `|` to the line before it.
    if p.peek() == Some(SyntaxKind::Newline) {
        bump_newlines(p);
    }
    let shape = argument_shape(p);
    // Nu rejects a keyword syntax with an item past its own, such as a `|` or a redirection.
    if call_ends_statement(p) && keyword(p) {
        return;
    }
    // A head that is no item, such as a `|`, is bumped as the tokens after it are.
    if p.peek().is_some_and(SyntaxKind::is_item) {
        start_call(p, shape);
    } else {
        p.start(SyntaxKind::Call);
    }
    rest_of_statement(p, assigned, |p| item(p, shape));
    p.finish();
}

/// Whether the items a call takes run to the statement's end.
fn call_ends_statement(p: &Parser<'_>) -> bool {
    p.remaining_kinds()
        .filter(|&kind| !p.is_skipped(kind))
        .find(|&kind| ends_call(kind))
        .is_none_or(ends_statement)
}
