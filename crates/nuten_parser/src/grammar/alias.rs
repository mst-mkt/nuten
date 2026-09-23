use nuten_syntax::SyntaxKind;

use super::{
    assignment::Assigned,
    command::{argument_shape, is_math_expression_like, start_call},
    ends_call, ends_statement, item,
    keyword::keyword,
    pipeline::{bump_newlines, next_line_starts_with_pipe, plain_pipe},
};
use crate::parser::Parser;

/// Whether the element ahead is an alias nu declares: `alias`, a name, `=`, and what follows.
/// Nu declares none where a value heads what follows `=`, and rejects the statement instead.
pub(super) fn alias_ahead(p: &Parser<'_>) -> bool {
    p.peek_text() == Some("alias")
        && p.nth(1).is_some_and(SyntaxKind::is_item)
        && p.nth(2) == Some(SyntaxKind::Assign)
        && p.nth_text(2) == Some("=")
        && match p.nth(3) {
            Some(SyntaxKind::Newline) => next_line_starts_with_pipe(p),
            kind => kind.is_some_and(|kind| !ends_statement(kind)),
        }
        && p.nth_text(3)
            .is_some_and(|head| !is_math_expression_like(head))
}

/// The `Alias` ahead: `alias`, the name, and `=`, which `alias_ahead` takes, then what follows.
/// `assigned` tells where the statement ends, as for a pipeline.
pub(super) fn alias(p: &mut Parser<'_>, assigned: Assigned) {
    p.start(SyntaxKind::Alias);
    for _ in 0..3 {
        p.bump();
    }
    replacement(p, assigned);
    p.finish();
}

/// What the alias stands for, as nu's `parse_call` reads the rest of the statement.
/// Nu has taken every token up to the statement's end, so a `|` or a redirection is an argument.
/// So is `||`, which nu rejects only where it splits a pipeline.
fn replacement(p: &mut Parser<'_>, assigned: Assigned) {
    // Nu's lexer joins a line that starts with `|` to the line before it.
    if p.peek() == Some(SyntaxKind::Newline) {
        bump_newlines(p);
    }
    // Nu reads no attribute in what an alias stands for.
    let shape = argument_shape(p, false);
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
    while let Some(kind) = p.peek() {
        match kind {
            SyntaxKind::Pipe => plain_pipe(p, assigned),
            SyntaxKind::Newline if next_line_starts_with_pipe(p) => bump_newlines(p),
            kind if ends_statement(kind) => break,
            kind if kind.is_item() => item(p, shape),
            _ => p.bump(),
        }
    }
    p.finish();
}

/// Whether the items a call takes run to the statement's end.
fn call_ends_statement(p: &Parser<'_>) -> bool {
    p.remaining_kinds()
        .filter(|&kind| !p.is_skipped(kind))
        .find(|&kind| ends_call(kind))
        .is_none_or(ends_statement)
}
