use super::Shape;
use crate::{ParseErrorKind, lexer::Mode, parser::Parser};

/// A `$` before a quote starts an interpolated string, not a variable.
pub(super) fn reads_dollar(text: &str, shape: Shape) -> bool {
    shape.takes_value()
        && text
            .strip_prefix('$')
            .is_some_and(|rest| !rest.starts_with(['"', '\'']))
}

pub(super) fn variable_error(p: &Parser<'_>, shape: Shape) -> Option<ParseErrorKind> {
    let text = p.peek_text()?;
    if !reads_dollar(text, shape) || is_variable_name(&text[1..]) {
        return None;
    }
    let relexed = p.relexed(Mode::CellPath)?;
    if relexed.inner().remaining_kinds().count() != 1 {
        return None;
    }
    Some(if text == "$" {
        ParseErrorKind::MissingVariableName
    } else {
        ParseErrorKind::InvalidVariableName
    })
}

/// Ported from nu's `is_variable`.
pub(super) fn is_variable_name(name: &str) -> bool {
    !name.is_empty()
        && !name.contains([
            '.', '[', '(', '{', '+', '-', '*', '^', '%', '/', '=', '!', '<', '>', '&', '|',
        ])
}

#[cfg(test)]
mod tests {
    use super::is_variable_name;

    #[test]
    fn names_are_what_nu_takes_after_dollar() {
        for name in ["a", "in", "a_b", "1", "a:b", "a]", "a)", "a}", "$", "日本"] {
            assert!(is_variable_name(name), "{name:?}");
        }
        for name in [
            "", "a.b", "a[0]", "a(b)", "a{b}", "a+b", "a-b", "a*b", "a^b", "a%b", "a/b", "a=b",
            "a!b", "a<b", "a>b", "a&b", "a|b",
        ] {
            assert!(!is_variable_name(name), "{name:?}");
        }
    }
}
