//! Compares the value nu reads from NUON with the one nuten reads.

use std::collections::BTreeSet;

use jsonc_parser::JsonValue;
use nuten_config::{
    ConfigError, ConfigErrorKind, Format,
    value::{Value, ValueKind},
};

use crate::{Checked, ast, nu};

pub(crate) fn check(src: &str) -> Result<Checked, String> {
    verdict(nuten_config::read(src, Format::Nuon), nu::from_nuon(src))
}

/// Nuten must read the value nu reads, or reject what nu rejects.
/// Where nu reads what nuten leaves unread, the file is skipped.
/// `nu` is the JSON nu writes of what it reads, or why it fails.
fn verdict(
    nuten: Result<Value, Vec<ConfigError>>,
    nu: Result<String, String>,
) -> Result<Checked, String> {
    match (nuten, nu) {
        (Ok(value), Ok(json)) => match difference(&ast::json(&json)?, &value) {
            Some((pointer, nu, nuten)) => Err(format!(
                "nu and nuten read differently at {pointer:?}: {} against {}",
                ast::excerpt(nu.clone()),
                excerpt(nuten)
            )),
            None => Ok(Checked::Passed),
        },
        (Ok(_), Err(error)) => Err(format!("nuten reads what nu rejects: {error}")),
        (Err(_), Err(_)) => Ok(Checked::Passed),
        (Err(errors), Ok(_)) => {
            let message = errors[0].kind.to_string();
            let unread = errors.iter().all(|error| {
                matches!(
                    error.kind,
                    ConfigErrorKind::Nuon(_) | ConfigErrorKind::Unreadable
                )
            });
            if unread {
                Ok(Checked::Skipped(format!(
                    "nuten does not read it: {message}"
                )))
            } else {
                Err(format!("nuten rejects what nu reads: {message}"))
            }
        },
    }
}

/// Where `nu` and `nuten` first differ, as a JSON pointer, with the two values there.
/// The order of keys is left aside, since the parser of nu's JSON keeps none.
fn difference<'n, 'j, 'v>(
    nu: &'n JsonValue<'j>,
    nuten: &'v Value,
) -> Option<(String, &'n JsonValue<'j>, &'v Value)> {
    match (nu, &nuten.kind) {
        (JsonValue::Object(object), ValueKind::Record(fields)) => {
            let same_keys = object.len() == fields.len()
                && fields.iter().all(|field| object.get(&field.key).is_some());
            if !same_keys {
                return Some((String::new(), nu, nuten));
            }
            fields.iter().find_map(|field| {
                let (pointer, nu, nuten) = difference(
                    object.get(&field.key).expect("the keys are the same"),
                    &field.value,
                )?;
                Some((format!("/{}{pointer}", field.key), nu, nuten))
            })
        },
        (JsonValue::Array(array), ValueKind::List(items)) if array.len() == items.len() => array
            .iter()
            .zip(items)
            .enumerate()
            .find_map(|(index, (nu, nuten))| {
                let (pointer, nu, nuten) = difference(nu, nuten)?;
                Some((format!("/{index}{pointer}"), nu, nuten))
            }),
        (nu, kind) => (!same_scalar(nu, kind)).then_some((String::new(), nu, nuten)),
    }
}

fn same_scalar(nu: &JsonValue<'_>, nuten: &ValueKind) -> bool {
    match (nu, nuten) {
        (JsonValue::String(nu), ValueKind::String(nuten)) => nu == nuten,
        (JsonValue::Number(nu), ValueKind::Int(nuten)) => nu.parse() == Ok(*nuten),
        // Nuten reads no float from NUON yet; these keep the comparison whole for when it does.
        (JsonValue::Number(nu), ValueKind::Float(nuten)) => nu.parse() == Ok(*nuten),
        // Nu writes a float that is not finite as null.
        (JsonValue::Null, ValueKind::Float(nuten)) => !nuten.is_finite(),
        (JsonValue::Boolean(nu), ValueKind::Bool(nuten)) => nu == nuten,
        (JsonValue::Null, ValueKind::Null) => true,
        _ => false,
    }
}

/// `value` in one line, in nuten's words for what `ast::excerpt` writes of nu's side.
fn excerpt(value: &Value) -> String {
    match &value.kind {
        ValueKind::Record(fields) => {
            let keys: BTreeSet<_> = fields.iter().map(|field| field.key.as_str()).collect();
            format!("a record with keys {keys:?}")
        },
        ValueKind::List(items) => format!("a list of length {}", items.len()),
        ValueKind::String(text) => format!("{text:?}"),
        ValueKind::Int(int) => int.to_string(),
        ValueKind::Float(float) => float.to_string(),
        ValueKind::Bool(boolean) => boolean.to_string(),
        ValueKind::Null => "null".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use jsonc_parser::JsonValue;
    use nuten_config::{
        ConfigError, Format, read,
        value::{Value, ValueKind},
    };

    use super::{same_scalar, verdict};
    use crate::Checked;

    fn nuten(src: &str) -> Result<Value, Vec<ConfigError>> {
        read(src, Format::Nuon)
    }

    #[test]
    fn same_value_passes() {
        let src = "{a: [1, x, true, null], b: {}, 'c d': \"é\"}";
        let json = r#"{"b": {}, "c d": "é", "a": [1, "x", true, null]}"#;
        assert!(matches!(
            verdict(nuten(src), Ok(json.to_string())),
            Ok(Checked::Passed)
        ));
    }

    #[test]
    fn different_value_fails_at_first_difference() {
        for (src, json, message) in [
            (
                "{a: [1, x]}",
                r#"{"a": [1, "y"]}"#,
                r#"at "/a/1": "y" against "x""#,
            ),
            ("{a: 1}", r#"{"a": "1"}"#, r#"at "/a": "1" against 1"#),
            (
                "{a: [1]}",
                r#"{"a": [1, 2]}"#,
                r#"at "/a": an array of length 2 against a list of length 1"#,
            ),
            (
                "{a: 1}",
                r#"{"b": 1}"#,
                r#"at "": an object with keys {"b"} against a record with keys {"a"}"#,
            ),
        ] {
            let error = verdict(nuten(src), Ok(json.to_string())).unwrap_err();
            assert!(error.ends_with(message), "{src}: {error}");
        }
    }

    #[test]
    fn nuten_reading_what_nu_rejects_fails() {
        let checked = verdict(nuten("{a: 1}"), Err("nu fails".to_string()));
        assert_eq!(
            checked.unwrap_err(),
            "nuten reads what nu rejects: nu fails"
        );
    }

    #[test]
    fn both_rejecting_passes() {
        let checked = verdict(nuten("{a: 1, a: 2}"), Err("nu fails".to_string()));
        assert!(matches!(checked, Ok(Checked::Passed)));
    }

    #[test]
    fn nuten_leaving_unread_what_nu_reads_is_skipped() {
        let checked = verdict(nuten("{a: 0x64}"), Ok(r#"{"a": 100}"#.to_string()));
        assert!(matches!(checked, Ok(Checked::Skipped(_))));
    }

    #[test]
    fn nuten_rejecting_what_nu_reads_fails() {
        for src in ["{a: 1, a: 2}", "{}; 1", "{a: 0x1, b: {c: 1, c: 2}}"] {
            let checked = verdict(nuten(src), Ok("{}".to_string()));
            assert!(
                checked
                    .unwrap_err()
                    .starts_with("nuten rejects what nu reads"),
                "{src}"
            );
        }
    }

    #[test]
    fn nu_reading_more_keys_fails() {
        let checked = verdict(nuten("{a: 1}"), Ok(r#"{"a": 1, "b": 2}"#.to_string()));
        assert!(checked.unwrap_err().ends_with(
            r#"at "": an object with keys {"a", "b"} against a record with keys {"a"}"#
        ));
    }

    #[test]
    fn invalid_json_from_nu_fails() {
        let checked = verdict(nuten("{a: 1}"), Ok("{".to_string()));
        assert!(checked.unwrap_err().starts_with("nu wrote invalid JSON"));
    }

    #[test]
    fn float_is_same_as_its_number() {
        assert!(same_scalar(
            &JsonValue::Number("1.5"),
            &ValueKind::Float(1.5)
        ));
        assert!(!same_scalar(
            &JsonValue::Number("1.5"),
            &ValueKind::Float(2.5)
        ));
    }

    #[test]
    fn non_finite_float_is_same_as_null() {
        assert!(same_scalar(&JsonValue::Null, &ValueKind::Float(f64::NAN)));
        assert!(same_scalar(
            &JsonValue::Null,
            &ValueKind::Float(f64::INFINITY)
        ));
        assert!(!same_scalar(&JsonValue::Null, &ValueKind::Float(1.5)));
    }
}
