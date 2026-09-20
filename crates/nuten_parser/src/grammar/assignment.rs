use nuten_syntax::SyntaxKind;
use rowan::{TextRange, TextSize};

use super::{
    ends_element, ends_statement, is_file_redirection,
    pipeline::{bump_newlines, next_line_starts_with_pipe, pipeline},
};
use crate::{ParseError, lexer::Mode, parser::Parser};

/// Where the type ahead ends, after a name ending in `:`.
/// It comes with the first error of lexing that type as nu does.
/// Nu reads the type up to the assignment operator.
/// For `const` it leaves out redirections at either end.
/// `None` also when its tokens hold an error of their own.
pub(super) fn var_type_error(p: &Parser<'_>, keyword: &str) -> Option<(TextSize, ParseError)> {
    let mut tokens = p
        .remaining_tokens()
        .filter(|(token, _)| !token.kind.is_trivia());
    let (name, text) = tokens.next()?;
    if !text.ends_with(':') {
        return None;
    }
    let mut prev = name.kind;
    let mut range: Option<TextRange> = None;
    for (token, _) in tokens {
        if token.kind == SyntaxKind::Assign && !is_file_redirection(prev) {
            break;
        }
        if token.kind == SyntaxKind::Error {
            return None;
        }
        if keyword != "const" || !(is_file_redirection(token.kind) || is_file_redirection(prev)) {
            range = Some(range.map_or(token.range, |range| range.cover(token.range)));
        }
        prev = token.kind;
    }
    let range = range?;
    let error = p.lex_error(range, Mode::VarType)?;
    Some((range.end(), error.into()))
}

/// Whether an assignment operator comes before the element ends.
/// As in nu, the target of a file redirection is none.
pub(super) fn assignment_ahead(p: &Parser<'_>) -> bool {
    let mut prev = None;
    for kind in p
        .remaining_kinds()
        .filter(|&kind| !matches!(kind, SyntaxKind::Whitespace | SyntaxKind::Comment))
    {
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

/// What an assignment operator takes: the rest of the statement.
/// A line-start `|` goes on there as it does after a line-end one.
pub(super) fn assigned_pipeline(p: &mut Parser<'_>) {
    if p.peek() == Some(SyntaxKind::Newline) && next_line_starts_with_pipe(p) {
        bump_newlines(p);
    }
    if p.peek().is_some_and(|kind| !ends_statement(kind)) {
        pipeline(p, true);
    }
}
