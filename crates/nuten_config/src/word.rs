//! Nu tries the shapes of a value in a fixed order and takes the first that reads the word.
//! A shape that finds the word malformed, as `1-2b` is for a filesize, rejects it outright.
//! So what a word reads as here is what nu's `parse_value` decides for `SyntaxShape::Any`.

use crate::value::ValueKind;

/// Nu rejects a `:` in the value of a record, but reads one in an item of a list.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Place {
    RecordValue,
    Item,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Word {
    Value(ValueKind),
    Rejected,
}

/// In the order nu looks for them in the word made uppercase.
const FILESIZE_UNITS: [&str; 13] = [
    "KB", "MB", "GB", "TB", "PB", "EB", "KIB", "MIB", "GIB", "TIB", "PIB", "EIB", "B",
];
/// In the order nu looks for them.
/// Both the micro sign and the Greek mu spell a microsecond.
const DURATION_UNITS: [&str; 10] = [
    "ns", "us", "\u{b5}s", "\u{3bc}s", "ms", "sec", "min", "hr", "day", "wk",
];

pub(crate) fn read(text: &str, place: Place) -> Word {
    match text {
        "true" => return Word::Value(ValueKind::Bool(true)),
        "false" => return Word::Value(ValueKind::Bool(false)),
        "null" => return Word::Value(ValueKind::Null),
        _ => {},
    }
    // A `$` heads a variable, which NUON has none of.
    // A quote or `r#` heads a string that has more after its closing quote.
    if text.starts_with(['$', '"', '\'']) || text.starts_with("r#") {
        return Word::Rejected;
    }
    // Nu reads a list or a record from a bracket, never a string, so the parser left this unread.
    if text.starts_with(['[', '{']) {
        return Word::Rejected;
    }
    if let Some(word) = typed_by_unit(text) {
        return word;
    }
    if is_datetime(text) {
        return Word::Value(ValueKind::Date);
    }
    if let Some(word) = number(text) {
        return word;
    }
    // A `(` makes an interpolation, which NUON has none of either.
    if text.contains('(') || (place == Place::RecordValue && text.contains(':')) {
        return Word::Rejected;
    }
    Word::Value(ValueKind::String(unquoted(text).to_string()))
}

/// The word without the backticks around it, as nu trims a pair of quotes off any word.
/// A word cannot start with the other quotes, since it would be rejected before.
fn unquoted(text: &str) -> &str {
    match text.as_bytes() {
        [b'`', .., b'`'] => &text[1..text.len() - 1],
        _ => text,
    }
}

fn typed_by_unit(text: &str) -> Option<Word> {
    let bytes = text.as_bytes();
    let heads_like_number = bytes.len() >= 2
        && (bytes[0].is_ascii_digit()
            || (matches!(bytes[0], b'.' | b'-') && bytes[1].is_ascii_digit()));
    if !heads_like_number {
        return None;
    }
    // The hex digit `b` would be taken for the unit `b`.
    let filesize = if text.starts_with("0x") {
        None
    } else {
        number_before_unit(
            &text.to_ascii_uppercase(),
            &FILESIZE_UNITS,
            ValueKind::Filesize,
        )
    };
    filesize.or_else(|| number_before_unit(text, &DURATION_UNITS, ValueKind::Duration))
}

fn number_before_unit(text: &str, units: &[&str], kind: ValueKind) -> Option<Word> {
    let unit = units.iter().find(|unit| text.ends_with(*unit))?;
    let number = without_underscores(&text[..text.len() - unit.len()]);
    if number.ends_with('$') {
        return None;
    }
    Some(if number.parse::<f64>().is_ok() {
        Word::Value(kind)
    } else {
        Word::Rejected
    })
}

fn is_datetime(text: &str) -> bool {
    let bytes = text.as_bytes();
    let heads_like_date =
        bytes.len() >= 6 && bytes[..4].iter().all(u8::is_ascii_digit) && bytes[4] == b'-';
    heads_like_date
        && (is_rfc3339(text)
            || is_rfc3339(&format!("{text}T00:00:00+00:00"))
            || is_rfc3339(&format!("{text}+00:00")))
}

/// Mirrors chrono's `parse_from_rfc3339`, which nu reads dates with.
fn is_rfc3339(text: &str) -> bool {
    let bytes = text.as_bytes();
    let Some([year, month, day, hour, minute, second]) = date_time_fields(bytes) else {
        return false;
    };
    let punctuated = bytes[4] == b'-'
        && bytes[7] == b'-'
        && matches!(bytes[10], b'T' | b't' | b' ')
        && bytes[13] == b':'
        && bytes[16] == b':';
    let in_range = (1..=12).contains(&month)
        && (1..=days_in_month(year, month)).contains(&day)
        && hour <= 23
        && minute <= 59
        && second <= 60;
    punctuated && in_range && without_fraction(&text[19..]).is_some_and(is_zone)
}

fn date_time_fields(bytes: &[u8]) -> Option<[u32; 6]> {
    Some([
        digits(bytes, 0, 4)?,
        digits(bytes, 5, 2)?,
        digits(bytes, 8, 2)?,
        digits(bytes, 11, 2)?,
        digits(bytes, 14, 2)?,
        digits(bytes, 17, 2)?,
    ])
}

fn without_fraction(rest: &str) -> Option<&str> {
    match rest.strip_prefix('.') {
        Some(fraction) => {
            let digits = fraction.bytes().take_while(u8::is_ascii_digit).count();
            (digits > 0).then(|| &fraction[digits..])
        },
        None => Some(rest),
    }
}

/// Chrono takes `−` (U+2212) as the sign of an offset too.
fn is_zone(rest: &str) -> bool {
    if matches!(rest, "Z" | "z") {
        return true;
    }
    let Some(offset) = ['+', '-', '\u{2212}']
        .into_iter()
        .find_map(|sign| rest.strip_prefix(sign))
    else {
        return false;
    };
    let bytes = offset.as_bytes();
    bytes.len() == 5
        && bytes[2] == b':'
        && matches!(
            (digits(bytes, 0, 2), digits(bytes, 3, 2)),
            (Some(hours), Some(minutes)) if hours <= 23 && minutes <= 59
        )
}

fn digits(bytes: &[u8], at: usize, count: usize) -> Option<u32> {
    bytes
        .get(at..at + count)
        .filter(|digits| digits.iter().all(u8::is_ascii_digit))
        .map(|digits| {
            digits
                .iter()
                .fold(0, |number, digit| number * 10 + u32::from(digit - b'0'))
        })
}

fn days_in_month(year: u32, month: u32) -> u32 {
    let leap = year.is_multiple_of(4) && (!year.is_multiple_of(100) || year.is_multiple_of(400));
    match month {
        2 if leap => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    }
}

fn number(text: &str) -> Option<Word> {
    let digits = without_underscores(text);
    let radix = [("0b", 2), ("0o", 8), ("0x", 16)]
        .into_iter()
        .find_map(|(prefix, radix)| digits.strip_prefix(prefix).map(|rest| (rest, radix)));
    if let Some((rest, radix)) = radix {
        // As `u64` cast to `i64`, so that `0xffffffffffffffff` reads as `-1` and not as too large.
        #[expect(clippy::cast_possible_wrap, reason = "the wrap is what nu does")]
        let int = u64::from_str_radix(rest, radix).map(|int| int as i64);
        return Some(int.map_or(Word::Rejected, |int| Word::Value(ValueKind::Int(int))));
    }
    if let Ok(int) = digits.parse::<i64>() {
        return Some(Word::Value(ValueKind::Int(int)));
    }
    digits
        .parse::<f64>()
        .ok()
        .map(|float| Word::Value(ValueKind::Float(float)))
}

/// Nu drops every `_` before it reads a number, so `_1_` is `1` and `n_an` is a float.
fn without_underscores(text: &str) -> String {
    text.chars().filter(|character| *character != '_').collect()
}

#[cfg(test)]
mod tests {
    use super::{Place, Word, is_datetime, read};
    use crate::value::ValueKind;

    fn item(text: &str) -> Word {
        read(text, Place::Item)
    }

    fn string(text: &str) -> Word {
        Word::Value(ValueKind::String(text.to_string()))
    }

    #[test]
    fn keyword_is_bool_or_null() {
        assert_eq!(item("true"), Word::Value(ValueKind::Bool(true)));
        assert_eq!(item("false"), Word::Value(ValueKind::Bool(false)));
        assert_eq!(item("null"), Word::Value(ValueKind::Null));
        assert_eq!(item("True"), string("True"));
        assert_eq!(item("nulls"), string("nulls"));
    }

    #[test]
    fn word_no_other_shape_reads_is_string() {
        for text in [
            "a",
            "a-b",
            "a_b",
            "_",
            "-",
            "+",
            "-x",
            "--",
            "+x",
            "*.nu",
            "**/*.nuon",
            "script.nu",
            "a.b.c",
            ".hidden",
            ".",
            "..",
            "...",
            "...a",
            "a..b",
            "../x",
            "~/x",
            "~",
            "/",
            "./",
            "x?y",
            "x=y",
            "a[0]",
            "a{b}",
            "a#b",
            "a\\b",
            "a'b'",
            "a\"b\"c",
            "`a`b",
            "!a",
            "%s",
            "@a",
            "a$b",
            "&",
            "^a",
            "<a>",
            "a>b",
            "é",
            "日本",
            "1.0.0",
            "1-2",
            "1s",
            "1mo",
            "1secs",
            "5minutes",
            "1kb2",
            "kb",
            "1e",
            "e5",
            "1.0.",
            "1e5.5",
            "0XFF",
            "0B1",
            "v1",
            "+1kb",
            "+0x1",
            "2024-1-1",
            "2024-01-01-x",
            "2024-02-30",
            "2023-02-29",
            "1900-02-29",
            "2100-02-29",
            "2024-04-31",
            "2024-06-31",
            "2024-09-31",
            "2024-11-31",
            "10000-01-01",
            "1kb$",
            "1Sec",
            "1..a",
            "a..1",
            "..x",
            "x..",
            "a..b..c",
        ] {
            assert_eq!(item(text), string(text), "{text}");
        }
    }

    #[test]
    fn dollar_between_number_and_unit_is_string() {
        for text in ["1$kb", "1$b", "1$sec", "1.5$kb"] {
            assert_eq!(item(text), string(text), "{text}");
        }
    }

    #[test]
    fn word_in_backticks_is_string_without_them() {
        assert_eq!(item("`a`b`"), string("a`b"));
        assert_eq!(item("``"), string(""));
    }

    #[test]
    fn colon_in_value_of_record_is_rejected_though_read_in_list() {
        assert_eq!(read("a:b", Place::RecordValue), Word::Rejected);
        assert_eq!(read("12:30", Place::RecordValue), Word::Rejected);
        assert_eq!(read("a:b", Place::Item), string("a:b"));
    }

    #[test]
    fn variable_interpolation_and_string_with_trailing_tokens_are_rejected() {
        for text in [
            "$x", "$", "$.a", "a(b)", "a(b)c", "\"a\"b", "'a'..3", "\"a\"..3", "r#'x'#y", "r#'x'##",
        ] {
            assert_eq!(item(text), Word::Rejected, "{text}");
        }
    }

    #[test]
    fn word_headed_by_bracket_is_rejected() {
        for text in ["[a;b]", "{a}"] {
            assert_eq!(item(text), Word::Rejected, "{text}");
        }
    }

    #[test]
    fn digits_with_sign_underscores_or_radix_are_int() {
        for (text, int) in [
            ("5", 5),
            ("-5", -5),
            ("+5", 5),
            ("05", 5),
            ("-0", 0),
            ("1_000", 1000),
            ("_1_", 1),
            ("1__", 1),
            ("-_1", -1),
            ("_-1", -1),
            ("0x10", 16),
            ("0xFF", 255),
            ("0x_1", 1),
            ("_0x1", 1),
            ("0b101", 5),
            ("0o17", 15),
            ("0xffffffffffffffff", -1),
            ("9223372036854775807", i64::MAX),
            ("-9223372036854775808", i64::MIN),
        ] {
            assert_eq!(item(text), Word::Value(ValueKind::Int(int)), "{text}");
        }
    }

    #[test]
    fn malformed_radix_digits_are_rejected() {
        for text in ["0xg", "0x", "0b2", "0o8", "0x1.5"] {
            assert_eq!(item(text), Word::Rejected, "{text}");
        }
    }

    #[test]
    fn number_with_fraction_exponent_or_beyond_int_is_float() {
        for (text, float) in [
            ("1.5", 1.5),
            ("1.", 1.0),
            (".5", 0.5),
            ("-.5", -0.5),
            ("1e5", 100_000.0),
            ("1E5", 100_000.0),
            ("1e+5", 100_000.0),
            ("1e-5", 0.000_01),
            ("1_._5", 1.5),
            ("9223372036854775808", 9_223_372_036_854_775_808.0),
            ("1e400", f64::INFINITY),
            ("inf", f64::INFINITY),
            ("+inf", f64::INFINITY),
            ("-inf", f64::NEG_INFINITY),
            ("Infinity", f64::INFINITY),
            ("inf_", f64::INFINITY),
        ] {
            assert_eq!(item(text), Word::Value(ValueKind::Float(float)), "{text}");
        }
        for text in ["nan", "NaN", "-nan", "n_an"] {
            assert!(
                matches!(item(text), Word::Value(ValueKind::Float(float)) if float.is_nan()),
                "{text}"
            );
        }
    }

    #[test]
    fn number_with_unit_is_filesize_or_duration() {
        for text in [
            "1kb", "1KB", "1Kb", "1kib", "1KiB", "2b", "2B", "1eb", "1pb", ".5kb", "-1kb", "1e3kb",
            "1.5kb", "1_kb", "1e400kb",
        ] {
            assert_eq!(item(text), Word::Value(ValueKind::Filesize), "{text}");
        }
        for text in [
            "1ns",
            "1us",
            "1\u{b5}s",
            "1\u{3bc}s",
            "1ms",
            "1sec",
            "10min",
            "1hr",
            "3day",
            "1wk",
            "1.5day",
            "1e3sec",
        ] {
            assert_eq!(item(text), Word::Value(ValueKind::Duration), "{text}");
        }
        assert_eq!(item("0x10b"), Word::Value(ValueKind::Int(267)));
    }

    #[test]
    fn malformed_number_before_unit_is_rejected() {
        for text in ["1-2b", "1.5.kb", "1.2.3kb", "0x10kb"] {
            assert_eq!(item(text), Word::Rejected, "{text}");
        }
    }

    #[test]
    fn date_with_or_without_time_and_offset_is_date() {
        for text in [
            "2024-01-01",
            "2024-02-29",
            "2000-02-29",
            "0000-01-01",
            "9999-12-31",
            "2024-01-01T00:00:00",
            "2024-01-01t00:00:00z",
            "2024-01-01T00:00:00Z",
            "2024-01-01T00:00:00+09:00",
            "2024-01-01T00:00:00-00:00",
            "2024-01-01T00:00:00\u{2212}05:00",
            "2024-01-01T00:00:00+23:59",
            "2024-01-01T23:59:60",
            "2024-01-01T00:00:60",
            "2024-01-01T00:00:00.5",
            "2024-01-01T00:00:00.123456789012",
        ] {
            assert_eq!(item(text), Word::Value(ValueKind::Date), "{text}");
        }
    }

    #[test]
    fn date_chrono_rejects_is_no_date() {
        for text in [
            "2024-1-1",
            "2024-13-01",
            "2024-00-10",
            "2024-01-32",
            "2024-01-00",
            "2024-02-30",
            "2023-02-29",
            "1900-02-29",
            "2100-02-29",
            "2024-04-31",
            "2024-01-01-x",
            "10000-01-01",
            "2024-01-01T24:00:00",
            "2024-01-01T00:60:00",
            "2024-01-01T00:00:61",
            "2024-01-01T00:00:00.",
            "2024-01-01T00:00:00+24:00",
            "2024-01-01T00:00:00+00:60",
            "2024-01-01T00:00:00+0000",
            "2024-01-01T00:00:00+05",
            "2024-01-01T00:00:00Zz",
            "2024-01-01T00:00",
        ] {
            assert!(!is_datetime(text), "{text}");
        }
    }
}
