use nuten_syntax::SyntaxKind;
use rowan::TextRange;

use super::{
    is_file_redirection,
    signature::{is_type, type_},
};
use crate::{lexer::Mode, parser::Parser};

/// `keyword` declares the variable: `let`, `mut`, `const`, or `for`.
pub(super) fn var_type(p: &mut Parser<'_>, keyword: &str) -> bool {
    let Some(var_type) = var_type_ahead(p, keyword) else {
        return false;
    };
    let end = var_type.range.end();
    if let Some(error) = p.lex_error(var_type.range, Mode::VarType) {
        p.bump();
        p.bump_error_to(end, error.into());
    } else if reads_as_type(p, &var_type) {
        p.bump();
        p.bump_relexed_to(end, Mode::VarType, type_);
    } else {
        return false;
    }
    true
}

struct VarType {
    range: TextRange,
    redirected: bool,
}

/// Nu reads the type up to the assignment operator, or up to the `in` of a `for`.
/// For `const` it leaves out redirections at either end.
fn var_type_ahead(p: &Parser<'_>, keyword: &str) -> Option<VarType> {
    let mut tokens = p
        .remaining_tokens()
        .filter(|(token, _)| !token.kind.is_trivia());
    let (name, text) = tokens.next()?;
    if !text.ends_with(':') {
        return None;
    }
    let mut prev = name.kind;
    let mut range: Option<TextRange> = None;
    let mut redirected = false;
    for (token, text) in tokens {
        let ends_type = if keyword == "for" {
            text == "in"
        } else {
            token.kind == SyntaxKind::Assign && !is_file_redirection(prev)
        };
        if ends_type {
            break;
        }
        if token.kind == SyntaxKind::Error {
            return None;
        }
        redirected |= is_file_redirection(token.kind);
        if keyword != "const" || !(is_file_redirection(token.kind) || is_file_redirection(prev)) {
            range = Some(range.map_or(token.range, |range| range.cover(token.range)));
        }
        prev = token.kind;
    }
    Some(VarType {
        range: range?,
        redirected,
    })
}

/// Emitting a type lexes every token after the name, taking in a redirection a `const` leaves out.
fn reads_as_type(p: &Parser<'_>, var_type: &VarType) -> bool {
    if var_type.redirected {
        return false;
    }
    let lexed = p.lexed_range(var_type.range, Mode::VarType);
    lexed.remaining_kinds().count() == 1 && is_type(&lexed)
}
