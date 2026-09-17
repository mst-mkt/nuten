//! Compares the ASTs nu writes as JSON.

use std::collections::{BTreeMap, BTreeSet};

use jsonc_parser::{JsonValue, Map, ParseOptions, parse_to_value};

/// Where the ASTs in `before` and `after` first differ, positions and line
/// endings aside, in one line; `None` when they are the same. Fails when
/// either is not JSON.
pub(crate) fn difference(before: &str, after: &str) -> Result<Option<String>, String> {
    let (before, after) = (json(before)?, json(after)?);
    let difference = first_difference(
        with_lf(without_positions(before)),
        with_lf(without_positions(after)),
    );
    Ok(difference.map(|(pointer, before, after)| {
        format!(
            "the AST differs at {pointer:?}: {} becomes {}",
            excerpt(before),
            excerpt(after)
        )
    }))
}

/// `value` in one line: an array by its length, an object by its keys, a
/// string quoted and cut to 200 characters, and anything else as JSON.
fn excerpt(value: JsonValue<'_>) -> String {
    match value {
        JsonValue::Array(array) => format!("an array of length {}", array.len()),
        JsonValue::Object(object) => {
            let keys: BTreeSet<_> = object.take_inner().into_keys().collect();
            format!("an object with keys {keys:?}")
        },
        JsonValue::String(text) => {
            let text = format!("{text:?}");
            match text.char_indices().nth(200) {
                Some((end, _)) => format!("{}…", &text[..end]),
                None => text,
            }
        },
        JsonValue::Number(number) => number.to_string(),
        JsonValue::Boolean(boolean) => boolean.to_string(),
        JsonValue::Null => "null".to_string(),
    }
}

fn json(text: &str) -> Result<JsonValue<'_>, String> {
    parse_to_value(text, &ParseOptions::default())
        .map_err(|error| format!("nu wrote invalid JSON: {error}"))?
        .ok_or_else(|| "nu wrote no JSON".to_string())
}

/// Without `start` and `end` wherever they stand together, and the
/// `span_source` beside them, which is the text between.
/// Nu writes positions as such pairs, in `span` objects and elsewhere, and as
/// the source excerpt it draws into the message of a `LabeledError`.
fn without_positions(value: JsonValue<'_>) -> JsonValue<'_> {
    match value {
        JsonValue::Object(object) => {
            let mut map = object.take_inner();
            if matches!(map.get("start"), Some(JsonValue::Number(_)))
                && matches!(map.get("end"), Some(JsonValue::Number(_)))
            {
                for key in ["start", "end", "span_source"] {
                    map.remove(key);
                }
            }
            let map: Map<_, _> = map
                .into_iter()
                .map(|(key, value)| {
                    let value = without_positions(value);
                    let value = if key == "LabeledError" {
                        without_excerpt(value)
                    } else {
                        value
                    };
                    (key, value)
                })
                .collect();
            JsonValue::Object(map.into())
        },
        JsonValue::Array(array) => {
            let items: Vec<_> = array.into_iter().map(without_positions).collect();
            JsonValue::Array(items.into())
        },
        other => other,
    }
}

/// `error`, the items of a `LabeledError`, without the lines of the excerpt
/// in its message, from the one that starts with `,-[` to the one that
/// starts with `` `---- ``.
fn without_excerpt(error: JsonValue<'_>) -> JsonValue<'_> {
    let JsonValue::Array(items) = error else {
        return error;
    };
    let mut items = items.take_inner();
    if let Some(JsonValue::String(message)) = items.first() {
        let mut inside = false;
        let lines: Vec<_> = message
            .lines()
            .filter(|line| {
                let line = line.trim_start();
                inside |= line.starts_with(",-[");
                let kept = !inside;
                inside &= !line.starts_with("`----");
                kept
            })
            .collect();
        items[0] = JsonValue::String(lines.join("\n").into());
    }
    JsonValue::Array(items.into())
}

/// With each `\r\n` in a string as `\n`, as the printer writes line endings
/// everywhere, inside strings too.
fn with_lf(value: JsonValue<'_>) -> JsonValue<'_> {
    match value {
        JsonValue::String(text) if text.contains("\r\n") => {
            JsonValue::String(text.replace("\r\n", "\n").into())
        },
        JsonValue::Object(object) => {
            let map: Map<_, _> = object
                .take_inner()
                .into_iter()
                .map(|(key, value)| (key, with_lf(value)))
                .collect();
            JsonValue::Object(map.into())
        },
        JsonValue::Array(array) => {
            let items: Vec<_> = array.into_iter().map(with_lf).collect();
            JsonValue::Array(items.into())
        },
        other => other,
    }
}

/// The first difference: a JSON pointer to it and the two values there.
/// Keys are compared sorted, so the difference reported does not depend on
/// nu's key order.
fn first_difference<'a>(
    a: JsonValue<'a>,
    b: JsonValue<'a>,
) -> Option<(String, JsonValue<'a>, JsonValue<'a>)> {
    match (a, b) {
        (JsonValue::Object(a), JsonValue::Object(b)) => {
            let a: BTreeMap<_, _> = a.take_inner().into_iter().collect();
            let mut b = b.take_inner();
            if a.len() != b.len() || !a.keys().all(|key| b.contains_key(key)) {
                let a: Map<_, _> = a.into_iter().collect();
                return Some((
                    String::new(),
                    JsonValue::Object(a.into()),
                    JsonValue::Object(b.into()),
                ));
            }
            a.into_iter().find_map(|(key, a)| {
                let (pointer, a, b) = first_difference(a, b.remove(&key).unwrap())?;
                Some((format!("/{key}{pointer}"), a, b))
            })
        },
        (JsonValue::Array(a), JsonValue::Array(b)) if a.len() == b.len() => a
            .into_iter()
            .zip(b)
            .enumerate()
            .find_map(|(index, (a, b))| {
                let (pointer, a, b) = first_difference(a, b)?;
                Some((format!("/{index}{pointer}"), a, b))
            }),
        (a, b) => (a != b).then_some((String::new(), a, b)),
    }
}

#[cfg(test)]
mod tests {
    use jsonc_parser::JsonValue;

    use super::{difference, first_difference, json, without_positions};

    fn value(text: &str) -> JsonValue<'_> {
        json(text).unwrap()
    }

    #[test]
    fn positions_are_dropped_with_the_text_between_them() {
        let ast = value(
            r#"{"span": {"start": 1, "end": 2, "span_source": "ab"}, "head": {"start": 3, "end": 4},
                "start": 5, "items": [{"start": 6, "end": 7}]}"#,
        );
        assert_eq!(
            without_positions(ast),
            value(r#"{"span": {}, "head": {}, "start": 5, "items": [{}]}"#)
        );
    }

    #[test]
    fn the_excerpt_in_the_message_of_a_labeled_error_is_dropped() {
        let error = |line: u32| {
            format!(
                concat!(
                    r#"{{"LabeledError": ["Error: x\n\n  x Not found\n   ,-[source:{line}:11]\n"#,
                    r#" {line} | const x = path self\n   :           ^^\n   `----\n  help: y\n", "#,
                    r#""z", {{"start": 1, "end": 2}}]}}"#,
                ),
                line = line
            )
        };
        assert_eq!(
            without_positions(value(&error(1))),
            value(r#"{"LabeledError": ["Error: x\n\n  x Not found\n  help: y", "z", {}]}"#)
        );
        assert_eq!(difference(&error(1), &error(3)), Ok(None));
    }

    #[test]
    fn the_first_difference_is_pointed_at() {
        let (pointer, a, b) = first_difference(
            value(r#"{"a": [1, {"b": "x"}], "c": true}"#),
            value(r#"{"a": [1, {"b": "y"}], "c": true}"#),
        )
        .unwrap();
        assert_eq!(pointer, "/a/1/b");
        assert_eq!((a, b), (value("\"x\""), value("\"y\"")));
        assert_eq!(first_difference(value("[1, 2]"), value("[1, 2]")), None);
        assert_eq!(
            first_difference(value(r#"{"a": 1, "b": 2}"#), value(r#"{"b": 2, "a": 1}"#)),
            None
        );
    }

    #[test]
    fn a_difference_prints_in_one_line() {
        let line = |before: &str, after: &str| difference(before, after).unwrap().unwrap();
        assert_eq!(
            line(
                r#"{"a": {"start": 1, "end": 2}, "b": 1}"#,
                r#"{"a": {"start": 3, "end": 4}, "b": 2}"#
            ),
            r#"the AST differs at "/b": 1 becomes 2"#
        );
        assert_eq!(
            line(r#"{"a": "x\ny"}"#, r#"{"a": "x"}"#),
            r#"the AST differs at "/a": "x\ny" becomes "x""#
        );
        assert_eq!(
            line("[1]", "[1, 2]"),
            r#"the AST differs at "": an array of length 1 becomes an array of length 2"#
        );
        assert_eq!(
            line(r#"{"a": 1}"#, r#"{"b": 1}"#),
            r#"the AST differs at "": an object with keys {"a"} becomes an object with keys {"b"}"#
        );
        assert_eq!(
            line(
                &format!(r#"{{"a": "{}"}}"#, "x".repeat(300)),
                r#"{"a": "y"}"#
            ),
            format!(
                r#"the AST differs at "/a": "{}… becomes "y""#,
                "x".repeat(199)
            )
        );
        assert_eq!(
            difference(
                r#"{"a": {"start": 1, "end": 2}}"#,
                r#"{"a": {"start": 3, "end": 4}}"#
            ),
            Ok(None)
        );
        assert!(difference("", "{}").is_err());
    }

    #[test]
    fn line_endings_do_not_count() {
        assert_eq!(
            difference(r#"{"a": ["x\r\ny"]}"#, r#"{"a": ["x\ny"]}"#),
            Ok(None)
        );
        assert!(
            difference(r#"{"a": "x\ry"}"#, r#"{"a": "x\ny"}"#)
                .unwrap()
                .is_some()
        );
    }
}
