//! Formatting must not change what nu parses: every `test_data/*.nu` must
//! give the same AST, positions aside, before and after formatting. Asks the
//! `nu` on PATH, which must be of the series the formatter targets; without
//! one, the test is skipped.

#![expect(clippy::print_stderr, reason = "a skipped test is reported")]

use std::{
    collections::BTreeMap,
    fs,
    io::{self, Write},
    path::Path,
    process::{Command, Stdio},
};

use jsonc_parser::{JsonValue, Map, ParseOptions, parse_to_value};
use nuten_format::{Config, format};

/// The series the formatter targets, as a prefix of `nu --version`.
const NU_VERSION: &str = "0.115.";

#[test]
fn formatting_keeps_the_ast_nu_parses() {
    let version = match Command::new("nu").arg("--version").output() {
        Ok(output) => String::from_utf8(output.stdout).unwrap(),
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            eprintln!("skipping the nu oracle: no nu on PATH");
            return;
        },
        Err(error) => panic!("nu --version: {error}"),
    };
    assert!(
        version.starts_with(NU_VERSION),
        "nu {} is not of the {NU_VERSION}x series",
        version.trim()
    );

    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("test_data");
    let mut paths: Vec<_> = fs::read_dir(dir)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .collect();
    paths.sort();
    let mut checked = 0;
    for path in paths {
        if path.extension().is_none_or(|extension| extension != "nu") {
            continue;
        }
        let name = path.file_name().unwrap().to_str().unwrap();
        let src = fs::read_to_string(&path).unwrap();
        let output = format(&src, &Config::default())
            .unwrap_or_else(|error| panic!("{name}: {:?}", error.errors));
        let (before, after) = (nu_ast(&src), nu_ast(&output));
        let (before, after) = (
            without_positions(json(&before)),
            without_positions(json(&after)),
        );
        if let Some((pointer, before, after)) = difference(before, after) {
            panic!(
                "{name}: the AST differs at {pointer:?}\n--- before\n{before:#?}\n\
                 --- after\n{after:#?}"
            );
        }
        checked += 1;
    }
    assert!(checked > 0, "no fixtures");
}

/// The block and the error nu parses `src` into, as one JSON text.
/// The block comes without its compiled IR, whose instruction order changes
/// from run to run.
fn nu_ast(src: &str) -> String {
    let mut nu = Command::new("nu")
        .args(["-n", "--stdin", "-c"])
        .arg(
            "ast $in --json | update block { from json | reject ir_block } \
             | update error { from json } | to json -r",
        )
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    nu.stdin.take().unwrap().write_all(src.as_bytes()).unwrap();
    let output = nu.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "nu failed on {src:?}:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap()
}

fn json(text: &str) -> JsonValue<'_> {
    parse_to_value(text, &ParseOptions::default())
        .unwrap()
        .expect("nu wrote a value")
}

/// Without `start` and `end` wherever they stand together, and the
/// `span_source` beside them, which is the text between.
/// nu writes positions as such pairs, in `span` objects and elsewhere.
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
                .map(|(key, value)| (key, without_positions(value)))
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

/// The first difference: a JSON pointer to it and the two values there.
/// Keys are compared sorted, so the difference reported does not depend on
/// nu's key order.
fn difference<'a>(
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
                let (pointer, a, b) = difference(a, b.remove(&key).unwrap())?;
                Some((format!("/{key}{pointer}"), a, b))
            })
        },
        (JsonValue::Array(a), JsonValue::Array(b)) if a.len() == b.len() => a
            .into_iter()
            .zip(b)
            .enumerate()
            .find_map(|(index, (a, b))| {
                let (pointer, a, b) = difference(a, b)?;
                Some((format!("/{index}{pointer}"), a, b))
            }),
        (a, b) => (a != b).then_some((String::new(), a, b)),
    }
}

#[test]
fn positions_are_dropped_with_the_text_between_them() {
    let value = json(
        r#"{"span": {"start": 1, "end": 2, "span_source": "ab"}, "head": {"start": 3, "end": 4},
            "start": 5, "items": [{"start": 6, "end": 7}]}"#,
    );
    assert_eq!(
        without_positions(value),
        json(r#"{"span": {}, "head": {}, "start": 5, "items": [{}]}"#)
    );
}

#[test]
fn the_first_difference_is_pointed_at() {
    let (a, b) = (
        json(r#"{"a": [1, {"b": "x"}], "c": true}"#),
        json(r#"{"a": [1, {"b": "y"}], "c": true}"#),
    );
    let (pointer, a, b) = difference(a, b).unwrap();
    assert_eq!(pointer, "/a/1/b");
    assert_eq!((a, b), (json("\"x\""), json("\"y\"")));
    assert_eq!(difference(json("[1, 2]"), json("[1, 2]")), None);
    assert_eq!(difference(json("[1]"), json("[1, 2]")).unwrap().0, "");
    assert_eq!(
        difference(json(r#"{"a": 1}"#), json(r#"{"b": 1}"#))
            .unwrap()
            .0,
        ""
    );
}
