//! Asks nu whether formatting keeps the value it reads from NUON.
//! Then compares that value with the one nuten reads.

use std::{collections::BTreeSet, path::Path};

use jsonc_parser::JsonValue;
use nuten_config::{
    ConfigError, ConfigErrorKind, Format,
    value::{Value, ValueKind},
};

use crate::{Checked, ast, nu, same_ast};

/// `output` is what formatting `src` gives, or `None` where syntax errors leave none.
/// `dir` is the directory nu parses `src` in.
pub(crate) fn check(
    src: &str,
    output: Option<&str>,
    dir: Option<&Path>,
) -> Result<Checked, String> {
    // Without output there is nothing to ask about formatting, but the readers may still disagree.
    let formatting = output.map(|output| same_value(src, output, dir));
    let reading = reading_verdict(nuten_config::read(src, Format::Nuon), nu::from_nuon(src));
    outcome(formatting, reading)
}

/// `formatting` is whether formatting keeps what nu reads, or `None` where that was not asked.
/// `reading` is whether nuten reads what nu reads.
/// A skip says whether formatting was asked about, so that the file does not look unchecked.
fn outcome(
    formatting: Option<Result<(), String>>,
    reading: Result<Checked, String>,
) -> Result<Checked, String> {
    let asked = formatting.transpose()?.is_some();
    match reading? {
        Checked::Skipped(reason) if asked => Ok(Checked::Skipped(format!(
            "formatting keeps what nu reads; {reason}"
        ))),
        checked => Ok(checked),
    }
}

/// Fails, in one line, when nu reads `before` and `after` into different values, or reads only one.
/// Where nu rejects both as NUON, the file is still nu code, so formatting must keep its AST.
fn same_value(before: &str, after: &str, dir: Option<&Path>) -> Result<(), String> {
    // Nu reads the same text into the same value.
    if before == after {
        return Ok(());
    }
    match formatting_verdict(nu::value_as_nuon(before), nu::value_as_nuon(after))? {
        Read::SameValue => Ok(()),
        Read::Rejected => same_ast(before, after, dir),
    }
}

/// What nu reads before and after formatting, where the two agree.
#[derive(Debug, PartialEq, Eq)]
enum Read {
    /// The same value.
    SameValue,
    /// No value from either, which leaves the AST to compare.
    Rejected,
}

/// Nu must read the same value before and after formatting, or reject both.
/// `before` and `after` are the NUON nu writes of what it reads, or why it fails.
fn formatting_verdict(
    before: Result<String, String>,
    after: Result<String, String>,
) -> Result<Read, String> {
    match (before, after) {
        (Ok(before), Ok(after)) if before == after => Ok(Read::SameValue),
        (Ok(before), Ok(after)) => {
            let at = differing_byte(&before, &after);
            Err(format!(
                "the value differs from byte {at}: {} becomes {}",
                ast::quoted(&before[at..]),
                ast::quoted(&after[at..])
            ))
        },
        (Err(_), Err(_)) => Ok(Read::Rejected),
        (Ok(_), Err(error)) => Err(format!("after formatting, {error}")),
        (Err(error), Ok(_)) => Err(format!(
            "nu reads the output, though not the source: {error}"
        )),
    }
}

/// The byte where `a` and `b` first differ, or the length of the shorter one.
fn differing_byte(a: &str, b: &str) -> usize {
    a.char_indices()
        .zip(b.chars())
        .find(|((_, a), b)| a != b)
        .map_or(a.len().min(b.len()), |((at, _), _)| at)
}

/// Nuten must read the value nu reads, or reject what nu rejects.
/// Where nu reads what nuten leaves unread, the file is skipped.
/// `nu` is the JSON nu writes of what it reads, or why it fails.
fn reading_verdict(
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
        ValueKind::String(text) => ast::quoted(text),
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

    use super::{Read, formatting_verdict, outcome, reading_verdict, same_scalar, same_value};
    use crate::{Checked, nu};

    fn nuten(src: &str) -> Result<Value, Vec<ConfigError>> {
        read(src, Format::Nuon)
    }

    /// What nu writes, or why it fails, as the verdicts take it.
    fn from_nu(written: Result<&str, &str>) -> Result<String, String> {
        written.map(str::to_string).map_err(str::to_string)
    }

    fn skipped(reason: &str) -> Checked {
        Checked::Skipped(reason.to_string())
    }

    #[test]
    fn value_differing_fails_before_reading_is_judged() {
        assert_eq!(
            outcome(Some(Err("v".to_string())), Err("r".to_string())),
            Err("v".to_string())
        );
    }

    #[test]
    fn skip_says_formatting_was_asked_about() {
        assert_eq!(
            outcome(Some(Ok(())), Ok(skipped("x"))),
            Ok(skipped("formatting keeps what nu reads; x"))
        );
        assert_eq!(outcome(None, Ok(skipped("x"))), Ok(skipped("x")));
    }

    #[test]
    fn reading_verdict_stands_when_value_is_kept() {
        assert_eq!(
            outcome(Some(Ok(())), Ok(Checked::Passed)),
            Ok(Checked::Passed)
        );
        assert_eq!(
            outcome(Some(Ok(())), Err("r".to_string())),
            Err("r".to_string())
        );
        assert_eq!(outcome(None, Ok(Checked::Passed)), Ok(Checked::Passed));
    }

    #[test]
    fn formatting_keeping_value_passes() {
        assert_eq!(
            formatting_verdict(from_nu(Ok("{a: 1}")), from_nu(Ok("{a: 1}"))),
            Ok(Read::SameValue)
        );
        assert_eq!(
            formatting_verdict(from_nu(Ok("")), from_nu(Ok(""))),
            Ok(Read::SameValue)
        );
    }

    #[test]
    fn formatting_changing_value_fails_from_first_differing_byte() {
        for (before, after, message) in [
            (
                "{a: [1, 2]}",
                "{a: [1, 3]}",
                r#"the value differs from byte 8: "2]}" becomes "3]}""#.to_string(),
            ),
            (
                "[1]",
                "[1, 2]",
                r#"the value differs from byte 2: "]" becomes ", 2]""#.to_string(),
            ),
            (
                "abc",
                "abcd",
                r#"the value differs from byte 3: "" becomes "d""#.to_string(),
            ),
            (
                "",
                "x",
                r#"the value differs from byte 0: "" becomes "x""#.to_string(),
            ),
            (
                "[é]",
                "[e]",
                r#"the value differs from byte 1: "é]" becomes "e]""#.to_string(),
            ),
            (
                &"x".repeat(300),
                &"y".repeat(300),
                format!(
                    r#"the value differs from byte 0: "{}… becomes "{}…"#,
                    "x".repeat(199),
                    "y".repeat(199)
                ),
            ),
        ] {
            assert_eq!(
                formatting_verdict(from_nu(Ok(before)), from_nu(Ok(after))).unwrap_err(),
                message,
                "{before:?} {after:?}"
            );
        }
    }

    #[test]
    fn nu_rejecting_source_and_output_leaves_ast_to_compare() {
        assert_eq!(
            formatting_verdict(from_nu(Err("before")), from_nu(Err("after"))),
            Ok(Read::Rejected)
        );
    }

    #[test]
    fn nu_rejecting_only_source_or_output_fails() {
        assert_eq!(
            formatting_verdict(from_nu(Ok("{a: 1}")), from_nu(Err("nu fails"))).unwrap_err(),
            "after formatting, nu fails"
        );
        assert_eq!(
            formatting_verdict(from_nu(Err("nu fails")), from_nu(Ok("{a: 1}"))).unwrap_err(),
            "nu reads the output, though not the source: nu fails"
        );
    }

    /// The crane build that runs the tests has no nu.
    fn nu_missing() -> bool {
        let missing = nu::check_version().is_err();
        if missing {
            let test = std::thread::current()
                .name()
                .unwrap_or_default()
                .to_string();
            eprintln!("skipping {test}: nu is not on PATH");
        }
        missing
    }

    #[test]
    fn same_value_tells_date_from_its_string() {
        if nu_missing() {
            return;
        }
        assert_eq!(
            same_value("2024-01-01", "\"2024-01-01T00:00:00+00:00\"", None).unwrap_err(),
            concat!(
                r#"the value differs from byte 0: "2024-01-01T00:00:00+00:00" "#,
                r#"becomes "\"2024-01-01T00:00:00+00:00\"""#
            )
        );
    }

    #[test]
    fn same_value_ignores_tokens_where_value_is_kept() {
        if nu_missing() {
            return;
        }
        assert_eq!(same_value("1kb", "1000b", None), Ok(()));
    }

    #[test]
    fn nu_rejecting_source_and_output_compares_their_asts() {
        if nu_missing() {
            return;
        }
        let error = same_value("{a: 1} | length", "{a: 1}; length", None).unwrap_err();
        assert!(error.contains("the AST differs"), "{error}");
        assert_eq!(
            same_value("{a: 1} | length", "{a: 1}  | length", None),
            Ok(())
        );
    }

    #[test]
    fn reading_same_value_passes() {
        let src = "{a: [1, x, true, null], b: {}, 'c d': \"é\"}";
        let json = r#"{"b": {}, "c d": "é", "a": [1, "x", true, null]}"#;
        assert!(matches!(
            reading_verdict(nuten(src), Ok(json.to_string())),
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
            let error = reading_verdict(nuten(src), Ok(json.to_string())).unwrap_err();
            assert!(error.ends_with(message), "{src}: {error}");
        }
    }

    #[test]
    fn nuten_reading_what_nu_rejects_fails() {
        let checked = reading_verdict(nuten("{a: 1}"), Err("nu fails".to_string()));
        assert_eq!(
            checked.unwrap_err(),
            "nuten reads what nu rejects: nu fails"
        );
    }

    #[test]
    fn both_rejecting_passes() {
        let checked = reading_verdict(nuten("{a: 1, a: 2}"), Err("nu fails".to_string()));
        assert!(matches!(checked, Ok(Checked::Passed)));
    }

    #[test]
    fn nuten_leaving_unread_what_nu_reads_is_skipped() {
        let checked = reading_verdict(nuten("{a: 0x64}"), Ok(r#"{"a": 100}"#.to_string()));
        assert!(matches!(checked, Ok(Checked::Skipped(_))));
    }

    #[test]
    fn nuten_rejecting_what_nu_reads_fails() {
        for src in ["{a: 1, a: 2}", "{}; 1", "{a: 0x1, b: {c: 1, c: 2}}"] {
            let checked = reading_verdict(nuten(src), Ok("{}".to_string()));
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
        let checked = reading_verdict(nuten("{a: 1}"), Ok(r#"{"a": 1, "b": 2}"#.to_string()));
        assert!(checked.unwrap_err().ends_with(
            r#"at "": an object with keys {"a", "b"} against a record with keys {"a"}"#
        ));
    }

    #[test]
    fn invalid_json_from_nu_fails() {
        let checked = reading_verdict(nuten("{a: 1}"), Ok("{".to_string()));
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
