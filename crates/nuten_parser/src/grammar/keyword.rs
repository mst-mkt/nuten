use super::{control::control, def::def, if_::if_, match_::match_};
use crate::parser::Parser;

/// The keyword syntax nu reads the call ahead as, if any.
/// Each syntax reads its node and returns whether it did.
/// Only a syntax that a call holds whole belongs here; `let` and the rest that reach past an assignment operator are read where the element is.
///
/// The keyword alone decides which syntax to try, so the shape of the items is that syntax's own to check.
pub(super) fn keyword(p: &mut Parser<'_>) -> bool {
    match p.peek_text() {
        Some("if") => if_(p),
        Some("for" | "while" | "loop" | "try") => control(p),
        Some("match") => match_(p),
        Some("def" | "extern" | "export") => def(p),
        _ => false,
    }
}
