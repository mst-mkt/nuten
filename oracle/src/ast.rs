//! Compares the ASTs nu writes as JSON.

use std::collections::{BTreeMap, BTreeSet};

use jsonc_parser::{JsonValue, Map, ParseOptions, parse_to_value};

/// Where what nu parses in `before` and `after` first differs, in one line.
/// Positions, line endings, and the layout in the flattened tokens are left aside.
/// `None` when the two are the same.
/// Fails when either is not JSON.
pub(crate) fn difference(before: &str, after: &str) -> Result<Option<String>, String> {
    let (before, after) = (json(before)?, json(after)?);
    let difference = first_difference(
        without_layout(with_lf(without_positions(before))),
        without_layout(with_lf(without_positions(after))),
    );
    Ok(difference.map(|(pointer, before, after)| {
        let what = if pointer.starts_with("/flat") {
            "the flattened tokens differ"
        } else {
            "the AST differs"
        };
        format!(
            "{what} at {pointer:?}: {} becomes {}",
            excerpt(before),
            excerpt(after)
        )
    }))
}

/// `value` in one line: an array by its length, an object by its keys.
/// A string comes quoted and cut to 200 characters, and anything else as JSON.
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

/// Without `start` and `end` wherever they stand together.
/// Without the `span_source` beside them, which is the text between.
/// Nu writes positions as such pairs, in `span` objects and elsewhere.
/// It writes them as the source excerpt it draws into the message of a `LabeledError` too.
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

/// `error`, the items of a `LabeledError`, without the lines of the excerpt in its message.
/// The excerpt runs from the line starting with `,-[` to the one starting with `` `---- ``.
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

/// With each `\n` in a string without the `\r`s before it.
/// The printer writes line endings everywhere, inside strings too.
/// With `\n` line endings it drops those `\r`s.
fn with_lf(value: JsonValue<'_>) -> JsonValue<'_> {
    match value {
        JsonValue::String(text) if text.contains("\r\n") => {
            let text = text
                .split('\n')
                .map(|line| line.trim_end_matches('\r'))
                .collect::<Vec<_>>()
                .join("\n");
            JsonValue::String(text.into())
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

/// Without the layout in the flattened tokens that formatting may change.
fn without_layout(value: JsonValue<'_>) -> JsonValue<'_> {
    let JsonValue::Object(object) = value else {
        return value;
    };
    let mut map = object.take_inner();
    if let Some(JsonValue::Array(tokens)) = map.remove("flat") {
        let tokens: Vec<_> = tokens
            .into_iter()
            .filter_map(token_without_layout)
            .collect();
        map.insert("flat".into(), JsonValue::Array(tokens.into()));
    }
    JsonValue::Object(map.into())
}

/// A token of a shape made of layout and words, normalized.
/// Such a shape is the brackets and the gaps between the items of a list.
/// A binary is one too: nu gives its brackets and items one token, and the AST holds its bytes.
/// Each run of whitespace and `,` becomes one space between two words, and nothing elsewhere.
/// `None` when that leaves the token empty.
/// A token of any other shape, such as a string, comes back as it is.
fn token_without_layout(token: JsonValue<'_>) -> Option<JsonValue<'_>> {
    const LAYOUT_SHAPES: [&str; 11] = [
        "shape_binary",
        "shape_block",
        "shape_closure",
        "shape_garbage",
        "shape_internalcall",
        "shape_keyword",
        "shape_list",
        "shape_match_pattern",
        "shape_record",
        "shape_signature",
        "shape_table",
    ];
    let JsonValue::Object(object) = token else {
        return Some(token);
    };
    let mut token = object.take_inner();
    if let (Some(JsonValue::String(shape)), Some(JsonValue::String(content))) =
        (token.get("shape"), token.get("content"))
        && LAYOUT_SHAPES.contains(&shape.as_ref())
    {
        let content = words(content);
        if content.is_empty() {
            return None;
        }
        token.insert("content".into(), JsonValue::String(content.into()));
    }
    Some(JsonValue::Object(token.into()))
}

/// `content` with each run of whitespace and `,` as one space between two words.
/// Such a run becomes nothing elsewhere.
/// A string that closes in `content` and a comment stay as they are.
/// A comment loses the whitespace it ends with, but keeps its line break.
/// A lone `\r` ends a comment as a line break does.
/// Nu ends a comment there when it looks for the closing bracket.
fn words(content: &str) -> String {
    let is_word = |char: char| {
        !matches!(
            char,
            '[' | ']' | '{' | '}' | '(' | ')' | '|' | ':' | ';' | '=' | '<' | '>' | '\n'
        )
    };
    let mut out = String::new();
    let mut after_gap = true;
    let mut rest = content;
    while let Some(char) = rest.chars().next() {
        if matches!(char, ' ' | '\t' | '\r' | '\n' | ',') {
            after_gap = true;
            rest = &rest[1..];
            continue;
        }
        let after_word = out.ends_with(is_word);
        if after_gap && after_word && is_word(char) {
            out.push(' ');
        }
        let comment = char == '#' && (after_gap || !after_word);
        let len = match char {
            _ if comment => rest.find(['\r', '\n']).map_or(rest.len(), |end| end + 1),
            '"' | '\'' | '`' => closing(rest).map_or(1, |close| close + 1),
            _ => char.len_utf8(),
        };
        let (part, after) = rest.split_at(len);
        if comment {
            out.push_str(part.trim_end_matches([' ', '\t', '\r', '\n']));
            if part.ends_with(['\r', '\n']) {
                out.push('\n');
            }
        } else {
            out.push_str(part);
        }
        after_gap = false;
        rest = after;
    }
    out
}

/// Where the string that `text` starts with closes, if it does.
/// It closes at a quote of the kind it opens with, not escaped when that is `"`.
fn closing(text: &str) -> Option<usize> {
    let quote = text.as_bytes()[0];
    let mut escaped = false;
    for (index, &byte) in text.as_bytes().iter().enumerate().skip(1) {
        if byte == quote && !escaped {
            return Some(index);
        }
        escaped = quote == b'"' && byte == b'\\' && !escaped;
    }
    None
}

/// The first difference: a JSON pointer to it and the two values there.
/// Keys are compared sorted, so the difference reported does not depend on nu's key order.
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

    use super::{difference, first_difference, json, without_layout, without_positions, words};

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
    fn excerpt_in_message_of_labeled_error_is_dropped() {
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
    fn layout_is_dropped_from_the_flattened_tokens_of_layout_shapes() {
        let ast = value(
            r#"{"block": {"content": " , "}, "flat": [
                {"content": "{ |x, y|\r\n\t", "shape": "shape_closure"},
                {"content": ", ", "shape": "shape_list"},
                {"content": "export  def", "shape": "shape_internalcall"},
                {"content": "[x: int, # a  b\r\n]", "shape": "shape_signature"},
                {"content": "0x[ff,  00 # c\n 01]", "shape": "shape_binary"},
                {"content": "a,  b\r\n", "shape": "shape_string"},
                {"content": "1"}]}"#,
        );
        assert_eq!(
            without_layout(ast),
            value(
                r#"{"block": {"content": " , "}, "flat": [
                    {"content": "{|x y|", "shape": "shape_closure"},
                    {"content": "export def", "shape": "shape_internalcall"},
                    {"content": "[x:int # a  b\n]", "shape": "shape_signature"},
                    {"content": "0x[ff 00 # c\n01]", "shape": "shape_binary"},
                    {"content": "a,  b\r\n", "shape": "shape_string"},
                    {"content": "1"}]}"#
            )
        );
    }

    #[test]
    fn words_stay_apart() {
        assert_ne!(words("{|x, y| "), words("{|xy| "));
        assert_ne!(words("[1, 2]"), words("[12]"));
        assert_eq!(words("{ |x| "), words("{|x|"));
        assert_eq!(words("[\n    a\n    b\n]"), words("[a, b]"));
        assert_eq!(words("[x: int=1]"), words("[x: int = 1]"));
        assert_ne!(words(r#"{|x = "a,  b"| "#), words(r#"{|x = "a b"| "#));
        assert_ne!(words(r#"["a \"  b"]"#), words(r#"["a \" b"]"#));
        assert_eq!(
            words("{ # it's  a, b  \n    "),
            words("{ # it's  a, b\n        ")
        );
        assert_ne!(words("[# a  b\n]"), words("[# a b\n]"));
        assert_ne!(words("[# c\n  x\n]"), words("[# c x\n]"));
        assert_eq!(words("[# c  \r\n\n  x\n]"), words("[\n    # c\n    x\n]"));
        assert_eq!(words("[a#b  c]"), words("[a#b c]"));
        assert_eq!(words("\"\n  }"), words("\"\n      }"));
        assert_eq!(words("'a  b' 日本  語"), "'a  b' 日本 語");
    }

    #[test]
    fn lone_carriage_return_ends_comment_as_line_break_does() {
        assert_eq!(words("[a # c\r]"), words("[\n    a # c\n]"));
        assert_eq!(words("[a # c\r\r\nb]"), words("[\n    a # c\n    b\n]"));
        assert_eq!(words("[a # c\rb]"), words("[\n    a # c\rb\n]"));
        assert_ne!(words("[a # c\rb]"), words("[a # c b]"));
    }

    #[test]
    fn first_difference_is_pointed_at() {
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
    fn difference_prints_in_one_line() {
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
            line(
                r#"{"flat": [{"content": "{|x, y| ", "shape": "shape_closure"}]}"#,
                r#"{"flat": [{"content": "{|xy| ", "shape": "shape_closure"}]}"#
            ),
            r#"the flattened tokens differ at "/flat/0/content": "{|x y|" becomes "{|xy|""#
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
            difference(
                r#"{"a": ["x\r\ny"], "flat": [{"content": "x\r\ny", "shape": "shape_string"}]}"#,
                r#"{"a": ["x\ny"], "flat": [{"content": "x\ny", "shape": "shape_string"}]}"#
            ),
            Ok(None)
        );
        assert_eq!(
            difference(r#"{"a": "x\r\r\ny"}"#, r#"{"a": "x\ny"}"#),
            Ok(None)
        );
        assert!(
            difference(r#"{"a": "x\ry"}"#, r#"{"a": "x\ny"}"#)
                .unwrap()
                .is_some()
        );
    }
}
