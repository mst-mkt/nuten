use super::{control::control, def::def, if_::if_, match_::match_, module::module};
use crate::parser::Parser;

/// The keyword syntax nu reads after an operator, as a call over the rest of the element.
/// Only `if` and `match` stand there; nu reads any other word as the term itself.
pub(super) fn keyword_after_operator(p: &mut Parser<'_>) -> bool {
    matches!(p.peek_text(), Some("if" | "match")) && keyword(p)
}

/// The keyword syntax nu reads the call ahead as, if any.
/// Each syntax reads its node and returns whether it did.
/// Only a syntax that a call holds whole belongs here.
/// `let` and the rest reach past an assignment operator, so they are read with the element.
///
/// The keyword alone decides which syntax to try.
/// The shape of the items is then that syntax's own to check.
pub(super) fn keyword(p: &mut Parser<'_>) -> bool {
    match p.peek_text() {
        Some("if") => if_(p),
        Some("for" | "while" | "loop" | "try") => control(p),
        Some("match") => match_(p),
        Some("def" | "extern" | "export") => def(p),
        Some("module") => module(p),
        _ => false,
    }
}
