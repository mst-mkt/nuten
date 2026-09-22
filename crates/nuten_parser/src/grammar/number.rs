/// Whether nu's `parse_number` reads `text` as a number: an int, or else a float.
/// Nu drops every `_` before reading either.
pub(super) fn is_number(text: &str) -> bool {
    int(text).is_some() || text.replace('_', "").parse::<f64>().is_ok()
}

/// The int nu's `parse_int` reads `text` as, if any.
pub(super) fn int(text: &str) -> Option<i64> {
    let text = text.replace('_', "");
    let prefixed = [("0b", 2), ("0o", 8), ("0x", 16)]
        .into_iter()
        .find_map(|(prefix, radix)| Some((text.strip_prefix(prefix)?, radix)));
    match prefixed {
        #[expect(
            clippy::cast_possible_wrap,
            reason = "nu takes the bits of a u64 for an i64"
        )]
        Some((digits, radix)) => u64::from_str_radix(digits, radix)
            .ok()
            .map(|int| int as i64),
        None => text.parse().ok(),
    }
}

#[cfg(test)]
mod tests {
    use super::int;

    #[test]
    fn ints_are_what_nu_reads_as_one() {
        for (text, expected) in [
            ("0", Some(0)),
            ("1_0", Some(10)),
            ("+1", Some(1)),
            ("-1", Some(-1)),
            ("-_1", Some(-1)),
            ("0x10", Some(16)),
            ("0b11", Some(3)),
            ("0o7", Some(7)),
            ("0xffffffffffffffff", Some(-1)),
            ("9223372036854775808", None),
            ("0b", None),
            ("_", None),
            ("1.5", None),
            ("a", None),
        ] {
            assert_eq!(int(text), expected, "{text:?}");
        }
    }
}
