use nuten_syntax::SyntaxKind;

use super::{
    ends_element, ends_statement, is_file_redirection,
    pipeline::{bump_newlines, next_line_starts_with_pipe, pipeline},
};
use crate::parser::Parser;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Assigned {
    No,
    AtLineBreak,
    AtSemicolon,
}

impl Assigned {
    pub(super) fn ends_at(self, kind: SyntaxKind) -> bool {
        match self {
            Assigned::No => false,
            Assigned::AtLineBreak => ends_statement(kind),
            Assigned::AtSemicolon => kind == SyntaxKind::Semicolon,
        }
    }
}

pub(super) fn assignment_ahead(p: &Parser<'_>) -> bool {
    let mut prev = None;
    for kind in p.remaining_kinds().filter(|&kind| !p.is_skipped(kind)) {
        if ends_element(kind) {
            return false;
        }
        if is_assignment_operator(kind, prev) {
            return true;
        }
        prev = Some(kind);
    }
    false
}

/// Nu reads an `=` after a file redirection as its target, not as an assignment.
pub(super) fn is_assignment_operator(kind: SyntaxKind, prev: Option<SyntaxKind>) -> bool {
    kind == SyntaxKind::Assign && !prev.is_some_and(is_file_redirection)
}

/// Nu lexes what an assignment operator takes again, with a line break ending a statement.
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

/// Line breaks with no statement after them stay outside, since a node must not end with trivia.
fn statement_ahead(p: &Parser<'_>) -> bool {
    p.remaining_kinds()
        .filter(|&kind| !p.is_skipped(kind))
        .find(|&kind| kind != SyntaxKind::Newline)
        .is_some_and(|kind| kind != SyntaxKind::Semicolon)
}
