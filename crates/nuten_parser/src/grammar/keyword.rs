use super::{
    Callee, at_call_item,
    control::control,
    def::def,
    if_::if_,
    match_::match_,
    module::{export_env, module},
    use_::use_,
    where_::where_,
};
use crate::parser::Parser;

/// Only `if` and `match` stand where nu takes a term; it reads any other word as the term itself.
pub(super) fn keyword_as_term(p: &mut Parser<'_>) -> bool {
    if !matches!(p.peek_text(), Some("if" | "match")) {
        return false;
    }
    // Nu accepts some calls the syntax does not read, as with a `--` or a variable for the body.
    // Telling them from the calls nu rejects would need the shapes nu reads there.
    // So any call the syntax does not read stays as written, not as terms lacking an operator.
    if !keyword(p, p.callee()) {
        while at_call_item(p) {
            p.bump();
        }
    }
    true
}

/// Only a syntax that a call holds whole belongs here.
/// `let` and the rest reach past an assignment operator, so they are read with the element.
/// Where a block around the call defines a command of its name, nu may run that instead.
/// A syntax may hold another with no brackets between them, as an `if` does in its condition.
/// So it counts toward the depth limit of brackets, past which the call stays a call.
pub(super) fn keyword(p: &mut Parser<'_>, callee: Callee) -> bool {
    if callee != Callee::Undefined || !p.can_nest() {
        return false;
    }
    p.deeper(|p| match p.peek_text() {
        Some("if") => if_(p),
        Some("for" | "while" | "loop" | "try") => control(p),
        Some("match") => match_(p),
        Some("def" | "extern") => def(p),
        Some("module") => module(p),
        Some("use") => use_(p),
        Some("where") => where_(p),
        Some("export") => exported(p),
        Some("export-env") => export_env(p),
        _ => false,
    })
}

/// Nu reads an `export` and the keyword after it as that keyword alone.
fn exported(p: &mut Parser<'_>) -> bool {
    match p.nth_text(1) {
        Some("def" | "extern") => def(p),
        Some("module") => module(p),
        Some("use") => use_(p),
        _ => false,
    }
}
