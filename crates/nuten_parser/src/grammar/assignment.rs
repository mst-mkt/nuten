use nuten_syntax::SyntaxKind;

use super::{
    ends_element, ends_statement, is_file_redirection,
    pipeline::{bump_newlines, next_line_starts_with_pipe, pipeline},
};
use crate::parser::Parser;

/// Where what an assignment operator takes ends, if the element holds one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Assigned {
    No,
    AtLineBreak,
    /// At a `;` or the end of the tokens.
    AtSemicolon,
}

/// Whether an assignment operator comes before the element ends.
/// As in nu, the target of a file redirection is none.
pub(super) fn assignment_ahead(p: &Parser<'_>) -> bool {
    let mut prev = None;
    for kind in p.remaining_kinds().filter(|&kind| !p.is_skipped(kind)) {
        if ends_element(kind) {
            return false;
        }
        if kind == SyntaxKind::Assign && !prev.is_some_and(is_file_redirection) {
            return true;
        }
        prev = Some(kind);
    }
    false
}

/// What an assignment operator takes: the statements of the rest of the element.
/// Nu lexes them again with a line break ending a statement, so they split at one.
/// Where the mode around the element skips a line break, the element runs to a `;` or the end.
/// A line-start `|` goes on there as it does after a line-end one.
pub(super) fn assigned_statements(p: &mut Parser<'_>) {
    p.with_newlines_read(|p, skipped| {
        let assigned = if skipped {
            Assigned::AtSemicolon
        } else {
            Assigned::AtLineBreak
        };
        if p.peek() == Some(SyntaxKind::Newline) && next_line_starts_with_pipe(p) {
            bump_newlines(p);
        }
        if p.peek().is_some_and(|kind| !ends_statement(kind)) {
            pipeline(p, assigned);
        }
        while assigned == Assigned::AtSemicolon && statement_ahead(p) {
            bump_newlines(p);
            pipeline(p, assigned);
        }
    });
}

/// Line breaks with no statement after them stay where they are.
/// So the node does not end with trivia.
fn statement_ahead(p: &Parser<'_>) -> bool {
    p.remaining_kinds()
        .filter(|&kind| !p.is_skipped(kind))
        .find(|&kind| kind != SyntaxKind::Newline)
        .is_some_and(|kind| kind != SyntaxKind::Semicolon)
}
