//! What a config file holds, whatever its format.

use nuten_syntax::{TextRange, TextSize};

use crate::{ConfigError, ConfigErrorKind};

#[derive(Debug, Clone, PartialEq)]
pub struct Value {
    pub kind: ValueKind,
    /// The whole value as written.
    pub range: TextRange,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ValueKind {
    /// No two fields have the same key.
    Record(Vec<Field>),
    List(Vec<Value>),
    String(String),
    Int(i64),
    /// A number with a fraction or an exponent, or, in JSON, an integer too large for an `Int`.
    Float(f64),
    Bool(bool),
    Null,

    /// This and the kinds below hold no value, since no setting takes one.
    Filesize,
    Duration,
    Date,
    Range,
    Binary,
    CellPath,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Field {
    pub key: String,
    /// The key as written, quotes included.
    pub key_range: TextRange,
    pub value: Value,
}

impl ValueKind {
    /// What the kind is called in an error.
    pub(crate) fn name(&self) -> &'static str {
        match self {
            ValueKind::Record(_) => "a record",
            ValueKind::List(_) => "a list",
            ValueKind::String(_) => "a string",
            ValueKind::Int(_) => "an integer",
            ValueKind::Float(_) => "a float",
            ValueKind::Bool(_) => "a boolean",
            ValueKind::Null => "null",
            ValueKind::Filesize => "a filesize",
            ValueKind::Duration => "a duration",
            ValueKind::Date => "a date",
            ValueKind::Range => "a range",
            ValueKind::Binary => "a binary",
            ValueKind::CellPath => "a cell path",
        }
    }
}

/// Null, as `from nuon` reads an empty file, so that such a file is reported as no record.
/// JSON has no empty document, but one is reported as no record all the same.
pub(crate) fn absent() -> Value {
    Value {
        kind: ValueKind::Null,
        range: TextRange::empty(TextSize::from(0)),
    }
}

/// A record of `fields`, or an error at each key a field before it has.
pub(crate) fn record(fields: Vec<Field>, range: TextRange) -> Result<Value, Vec<ConfigError>> {
    let duplicates: Vec<_> = fields
        .iter()
        .enumerate()
        .filter(|(index, field)| {
            fields[..*index]
                .iter()
                .any(|before| before.key == field.key)
        })
        .map(|(_, field)| ConfigError {
            kind: ConfigErrorKind::DuplicateKey(field.key.clone()),
            range: field.key_range,
        })
        .collect();
    if !duplicates.is_empty() {
        return Err(duplicates);
    }
    Ok(Value {
        kind: ValueKind::Record(fields),
        range,
    })
}

/// Every value of `results`, or every error among them when there is any.
pub(crate) fn all<T>(
    results: impl IntoIterator<Item = Result<T, Vec<ConfigError>>>,
) -> Result<Vec<T>, Vec<ConfigError>> {
    let (values, errors): (Vec<_>, Vec<_>) = results.into_iter().partition(Result::is_ok);
    if errors.is_empty() {
        Ok(values.into_iter().flatten().collect())
    } else {
        Err(errors
            .into_iter()
            .filter_map(Result::err)
            .flatten()
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use nuten_syntax::TextRange;

    use super::ValueKind;
    use crate::{Format, load, read, test_support::errors};

    #[test]
    fn every_later_duplicate_key_is_reported_at_key() {
        let text = r#"{"a": 1, "a": 2, "b": 3, "a": 4}"#;
        let reported: Vec<_> = load(text, Format::Json)
            .unwrap_err()
            .into_iter()
            .map(|error| (error.kind.to_string(), error.range))
            .collect();
        let at = |offset: u32| TextRange::at(offset.into(), 3.into());
        assert_eq!(
            reported,
            [
                ("duplicate key `a`".to_string(), at(9)),
                ("duplicate key `a`".to_string(), at(25)),
            ]
        );
    }

    #[test]
    fn duplicate_key_inside_nested_record_is_reported() {
        assert_eq!(
            errors(r#"{"a": {"b": 1, "b": 1}}"#, Format::Json),
            [("duplicate key `b`".to_string(), r#""b""#)]
        );
    }

    #[test]
    fn keys_differing_in_case_are_no_duplicates() {
        let text = r#"{"line_width": 80, "Line_width": 80}"#;
        assert_eq!(
            errors(text, Format::Json),
            [("unknown key `Line_width`".to_string(), r#""Line_width""#)]
        );
    }

    #[test]
    fn errors_inside_value_hide_those_around_it() {
        let text = r#"{"x": 0, "a": {"b": 1, "b": 1}, "a": 1}"#;
        assert_eq!(
            errors(text, Format::Json),
            [("duplicate key `b`".to_string(), r#""b""#)]
        );
    }

    #[test]
    fn errors_in_sibling_values_are_reported_in_order() {
        let text = r#"{"a": {"b": 1, "b": 1}, "c": [{"d": 1, "d": 1}]}"#;
        assert_eq!(
            errors(text, Format::Json),
            [
                ("duplicate key `b`".to_string(), r#""b""#),
                ("duplicate key `d`".to_string(), r#""d""#),
            ]
        );
    }

    #[test]
    fn empty_file_is_null_at_start() {
        for format in [Format::Json, Format::Jsonc, Format::Nuon] {
            let value = read("", format).unwrap();
            assert_eq!(value.kind, ValueKind::Null, "{format:?}");
            assert_eq!(value.range, TextRange::empty(0.into()), "{format:?}");
        }
    }
}
