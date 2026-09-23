use super::{
    at_call_item, call_items, control::control, def::def, if_::if_, is_flag, match_::match_,
    module::module, use_::use_, where_::where_,
};
use crate::parser::Parser;

/// The keyword syntax nu reads where it takes a term, as a call over the rest of the element.
/// Only `if` and `match` stand there; nu reads any other word as the term itself.
/// Returns whether it read the rest of the element.
pub(super) fn keyword_as_term(p: &mut Parser<'_>) -> bool {
    if !matches!(p.peek_text(), Some("if" | "match")) {
        return false;
    }
    if keyword(p) {
        return true;
    }
    // `if` and `match` read no flag, yet nu accepts a `--` or a help flag in their call.
    // So a call with one stays as written, not as terms that would each lack an operator.
    if !call_items(p).iter().any(|item| is_flag(item)) {
        return false;
    }
    while at_call_item(p) {
        p.bump();
    }
    true
}

/// The keyword syntax nu reads the call ahead as, if any.
/// Each syntax reads its node and returns whether it did.
/// Only a syntax that a call holds whole belongs here.
/// `let` and the rest reach past an assignment operator, so they are read with the element.
///
/// The keyword alone decides which syntax to try, or the one after an `export`.
/// The shape of the items is then that syntax's own to check.
pub(super) fn keyword(p: &mut Parser<'_>) -> bool {
    match p.peek_text() {
        Some("if") => if_(p),
        Some("for" | "while" | "loop" | "try") => control(p),
        Some("match") => match_(p),
        Some("def" | "extern") => def(p),
        Some("module") => module(p),
        Some("use") => use_(p),
        Some("where") => where_(p),
        Some("export") => exported(p),
        _ => false,
    }
}

/// An `export` and the keyword after it, which nu reads as that keyword alone.
/// The syntax of that keyword takes the `export` in.
fn exported(p: &mut Parser<'_>) -> bool {
    match p.nth_text(1) {
        Some("def" | "extern") => def(p),
        Some("module") => module(p),
        Some("use") => use_(p),
        _ => false,
    }
}
