use nuten_syntax::SyntaxKind;
use rowan::TextRange;

use super::is_file_redirection;
use crate::{lexer::Mode, parser::Parser};

/// The name ahead and the type after it, when that type is an `Error`.
/// `keyword` declares the variable: `let`, `mut`, `const`, or `for`.
/// Returns whether it emits them; otherwise they are left as they are.
pub(super) fn var_type(p: &mut Parser<'_>, keyword: &str) -> bool {
    let Some(range) = var_type_ahead(p, keyword) else {
        return false;
    };
    let Some(error) = p.lex_error(range, Mode::VarType) else {
        return false;
    };
    p.bump();
    p.bump_error_to(range.end(), error.into());
    true
}

/// Where the type ahead is, if the name ahead ends in `:`.
/// Nu reads the type up to the assignment operator, or up to the `in` of a `for`.
/// For `const` it leaves out redirections at either end.
/// `None` also when its tokens hold an error of their own.
fn var_type_ahead(p: &Parser<'_>, keyword: &str) -> Option<TextRange> {
    let mut tokens = p
        .remaining_tokens()
        .filter(|(token, _)| !token.kind.is_trivia());
    let (name, text) = tokens.next()?;
    if !text.ends_with(':') {
        return None;
    }
    let mut prev = name.kind;
    let mut range: Option<TextRange> = None;
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
        if keyword != "const" || !(is_file_redirection(token.kind) || is_file_redirection(prev)) {
            range = Some(range.map_or(token.range, |range| range.cover(token.range)));
        }
        prev = token.kind;
    }
    range
}
