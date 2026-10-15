use nuten_parser::is_number;

pub(crate) fn is_number_like(text: &str) -> bool {
    is_number(text) || is_unit_value(text) || is_date(text)
}

const FILESIZE_UNITS: &[&str] = &[
    "KB", "MB", "GB", "TB", "PB", "EB", "KIB", "MIB", "GIB", "TIB", "PIB", "EIB", "B",
];

const DURATION_UNITS: &[&str] = &[
    "ns", "us", "\u{b5}s", "\u{3bc}s", "ms", "sec", "min", "hr", "day", "wk",
];

fn is_unit_value(text: &str) -> bool {
    let starts_as_number = match text.as_bytes() {
        [first, _, ..] if first.is_ascii_digit() => true,
        [b'.' | b'-', second, ..] => second.is_ascii_digit(),
        _ => false,
    };
    if !starts_as_number {
        return false;
    }
    let upper = text.to_ascii_uppercase();
    let filesize = FILESIZE_UNITS
        .iter()
        .find_map(|unit| upper.strip_suffix(unit))
        .map(|number| &text[..number.len()]);
    let number = filesize.or_else(|| {
        DURATION_UNITS
            .iter()
            .find_map(|unit| text.strip_suffix(unit))
    });
    number.is_some_and(|number| number.replace('_', "").parse::<f64>().is_ok())
}

fn is_date(text: &str) -> bool {
    let (date, time) = match text.split_once(['T', 't']) {
        Some((date, time)) => (date, Some(time)),
        None => (text, None),
    };
    is_calendar_date(date) && time.is_none_or(is_time_with_offset)
}

fn is_calendar_date(text: &str) -> bool {
    let Some([year, month, day]) = fields(text, '-') else {
        return false;
    };
    let (Some(year), Some(month), Some(day)) = (digits(year, 4), digits(month, 2), digits(day, 2))
    else {
        return false;
    };
    let leap = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
    let days = match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if leap => 29,
        2 => 28,
        _ => return false,
    };
    (1..=days).contains(&day)
}

fn is_time_with_offset(text: &str) -> bool {
    let (time, offset) = if let Some(time) = text.strip_suffix(['Z', 'z']) {
        (time, None)
    } else if let Some(at) = text.rfind(['+', '-']) {
        (&text[..at], Some(&text[at + 1..]))
    } else {
        (text, None)
    };
    is_time(time) && offset.is_none_or(is_offset)
}

fn is_time(text: &str) -> bool {
    let (time, fraction) = match text.split_once('.') {
        Some((time, fraction)) => (time, Some(fraction)),
        None => (text, None),
    };
    let Some([hour, minute, second]) = fields(time, ':') else {
        return false;
    };
    let in_range = |field, max| digits(field, 2).is_some_and(|value| value <= max);
    in_range(hour, 23)
        && in_range(minute, 59)
        && in_range(second, 60)
        && fraction.is_none_or(|fraction| {
            !fraction.is_empty() && fraction.bytes().all(|byte| byte.is_ascii_digit())
        })
}

fn is_offset(text: &str) -> bool {
    let Some([hour, minute]) = fields(text, ':') else {
        return false;
    };
    digits(hour, 2).is_some_and(|hour| hour <= 23)
        && digits(minute, 2).is_some_and(|minute| minute <= 59)
}

fn fields<const N: usize>(text: &str, separator: char) -> Option<[&str; N]> {
    text.split(separator).collect::<Vec<_>>().try_into().ok()
}

fn digits(text: &str, len: usize) -> Option<u32> {
    (text.len() == len && text.bytes().all(|byte| byte.is_ascii_digit()))
        .then(|| text.parse().ok())
        .flatten()
}

#[cfg(test)]
mod tests {
    use super::is_number_like;

    #[test]
    fn number_with_any_unit_is_number_like() {
        for unit in [
            "b", "kb", "mb", "gb", "tb", "pb", "eb", "kib", "mib", "gib", "tib", "pib", "eib",
        ] {
            for spelling in [unit.to_owned(), unit.to_ascii_uppercase()] {
                assert!(is_number_like(&format!("1{spelling}")), "{spelling}");
            }
        }
        for unit in [
            "ns", "us", "\u{b5}s", "\u{3bc}s", "ms", "sec", "min", "hr", "day", "wk",
        ] {
            assert!(is_number_like(&format!("1{unit}")), "{unit}");
            assert!(!is_number_like(&format!("1{unit}x")), "{unit}");
        }
    }

    #[test]
    fn words_nu_reads_as_values_are_number_like() {
        for word in [
            "inf",
            "0x10b",
            "1Kb",
            "1EiB",
            "-1kb",
            ".5kb",
            "1.kb",
            "1_000kb",
            "1e3kb",
            "2024-01-01",
            "2024-02-29",
            "2000-02-29",
            "2024-01-01T00:00:00Z",
            "2024-01-01t00:00:00z",
            "2024-01-01T00:00:00",
            "2024-01-01T00:00:00.5+09:00",
            "2024-01-01T00:00:60Z",
        ] {
            assert!(is_number_like(word), "{word}");
        }
    }

    #[test]
    fn words_nu_reads_as_no_value_are_not_number_like() {
        for word in [
            "1Sec",
            "+1kb",
            "0b1kb",
            "2024-13-01",
            "2024-02-30",
            "2023-02-29",
            "1900-02-29",
            "2024-04-31",
            "2024-01",
            "2024-01-01T24:00:00",
            "2024-01-01T00:00",
            "2024-01-01T00:00:00+0900",
            "2024-01-01T00:00:00+24:00",
            "2024-01-01T00:00:00.Z",
        ] {
            assert!(!is_number_like(word), "{word}");
        }
    }
}
