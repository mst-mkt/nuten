use super::Shape;

/// Whether nu reads `text`, which starts a word, as a `$` and what follows it for `shape`.
/// A `$` before a quote starts an interpolated string instead.
pub(super) fn reads_dollar(text: &str, shape: Shape) -> bool {
    shape.takes_value()
        && text
            .strip_prefix('$')
            .is_some_and(|rest| !rest.starts_with(['"', '\'']))
}

/// Whether nu's `is_variable` takes `name`, the text after `$`.
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
    fn variable_names_are_what_nu_takes_after_dollar() {
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
