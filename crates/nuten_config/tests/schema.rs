//! The JSON Schema, and that what it allows is what the loader takes.

use jsonc_parser::{JsonObject, JsonValue, ParseOptions, parse_to_value};
use nuten_config::{Config, ConfigErrorKind, Format, load, schema};

fn root(text: &str) -> JsonObject<'_> {
    let Some(JsonValue::Object(root)) = parse_to_value(text, &ParseOptions::default()).unwrap()
    else {
        panic!("the schema is no object");
    };
    root
}

/// The properties of `object`, the schema of a record.
fn properties_of(mut object: JsonObject<'_>) -> Vec<(String, JsonObject<'_>)> {
    let Some(JsonValue::Object(properties)) = object.take("properties") else {
        panic!("the schema of a record has no properties");
    };
    properties
        .take_inner()
        .into_iter()
        .map(|(key, property)| match property {
            JsonValue::Object(property) => (key.into_owned(), property),
            _ => panic!("{key} is no object"),
        })
        .collect()
}

fn properties(text: &str) -> Vec<(String, JsonObject<'_>)> {
    properties_of(root(text))
}

/// The schema of the records in the list `property` describes, if its items are records.
fn record_items<'a>(property: &JsonObject<'a>) -> Option<JsonObject<'a>> {
    match property.get("items")? {
        JsonValue::Object(items)
            if items.get_string("type").map(AsRef::as_ref) == Some("object") =>
        {
            Some(items.clone())
        },
        _ => None,
    }
}

fn integer_edge(property: &JsonObject<'_>, edge: &str) -> i64 {
    property
        .get_number(edge)
        .expect("an integer has both edges")
        .parse()
        .unwrap()
}

type Loaded = Result<Config, Vec<ConfigErrorKind>>;

fn load_text(text: &str) -> Loaded {
    load(text, Format::Json).map_err(|errors| errors.into_iter().map(|error| error.kind).collect())
}

fn load_one(key: &str, value: &str) -> Loaded {
    load_text(&format!("{{\"{key}\": {value}}}"))
}

/// Loads `value` as that of `key` in a record of `overrides` that picks some files.
fn load_in_override(key: &str, value: &str) -> Loaded {
    let record = if key == "include" {
        format!("{{\"include\": {value}}}")
    } else {
        format!("{{\"include\": [\"x\"], \"{key}\": {value}}}")
    };
    load_one("overrides", &format!("[{record}]"))
}

fn allowed_values(key: &str, property: &JsonObject<'_>) -> Vec<String> {
    match property.get_string("type").map(AsRef::as_ref) {
        Some("integer") => ["minimum", "maximum"]
            .map(|edge| integer_edge(property, edge).to_string())
            .into(),
        Some("string") => vec!["\"x\"".to_string()],
        Some("boolean") => vec!["true".to_string(), "false".to_string()],
        Some("array") if record_items(property).is_some() => {
            vec!["[]".to_string(), "[{\"include\": [\"x\"]}]".to_string()]
        },
        Some("array") => vec!["[]".to_string(), "[\"x\", \"y/z\"]".to_string()],
        _ => property
            .get_array("enum")
            .expect("a property has a type or choices")
            .iter()
            .map(|choice| match choice {
                JsonValue::String(choice) => format!("\"{choice}\""),
                _ => panic!("{key} has a choice of no string"),
            })
            .collect(),
    }
}

/// Whether the errors of loading a value are the ones expected.
type Expected = fn(&[ConfigErrorKind]) -> bool;

/// Each value the schema rejects, with what the errors of loading it are.
fn rejected_values(property: &JsonObject<'_>) -> Vec<(String, Expected)> {
    let wrong_type: Expected = |errors| matches!(errors, [ConfigErrorKind::WrongType { .. }]);
    match property.get_string("type").map(AsRef::as_ref) {
        Some("integer") => {
            let out_of_range: Expected =
                |errors| matches!(errors, [ConfigErrorKind::OutOfRange { .. }]);
            let below = integer_edge(property, "minimum") - 1;
            let above = integer_edge(property, "maximum") + 1;
            vec![
                (below.to_string(), out_of_range),
                (above.to_string(), out_of_range),
            ]
        },
        Some("string") => vec![("1".to_string(), wrong_type)],
        Some("boolean") => vec![("\"true\"".to_string(), wrong_type)],
        Some("array") if record_items(property).is_some() => vec![
            ("\"x\"".to_string(), wrong_type),
            ("[1]".to_string(), wrong_type),
            ("[{}]".to_string(), |errors| {
                errors == [ConfigErrorKind::OverrideWithoutInclude]
            }),
        ],
        Some("array") => vec![
            ("\"x\"".to_string(), wrong_type),
            ("[1]".to_string(), wrong_type),
        ],
        _ => vec![("\"none of the choices\"".to_string(), |errors| {
            matches!(errors, [ConfigErrorKind::UnknownChoice { .. }])
        })],
    }
}

fn assert_allowed_values_load(
    properties: &[(String, JsonObject<'_>)],
    load: fn(&str, &str) -> Loaded,
) {
    for (key, property) in properties {
        for value in allowed_values(key, property) {
            assert!(load(key, &value).is_ok(), "{key}: {value}");
        }
    }
}

fn assert_rejected_values_fail(
    properties: &[(String, JsonObject<'_>)],
    load: fn(&str, &str) -> Loaded,
) {
    for (key, property) in properties {
        for (value, expected) in rejected_values(property) {
            let errors = load(key, &value).unwrap_err();
            assert!(expected(&errors), "{key}: {value}: {errors:?}");
        }
    }
}

fn override_properties(text: &str) -> Vec<(String, JsonObject<'_>)> {
    let items = properties(text)
        .iter()
        .find_map(|(key, property)| (key == "overrides").then(|| record_items(property)))
        .flatten()
        .expect("`overrides` is a list of records");
    properties_of(items)
}

#[test]
fn schema_allowed_value_loads() {
    let text = schema();
    assert_allowed_values_load(&properties(&text), load_one);
    assert_allowed_values_load(&override_properties(&text), load_in_override);
}

#[test]
fn schema_rejected_value_fails_to_load() {
    let text = schema();
    assert_rejected_values_fail(&properties(&text), load_one);
    assert_rejected_values_fail(&override_properties(&text), load_in_override);
}

#[test]
fn key_unknown_to_schema_fails_to_load() {
    let text = schema();
    assert_eq!(root(&text).get_boolean("additionalProperties"), Some(false));
    let keys: Vec<_> = properties(&text).into_iter().map(|(key, _)| key).collect();
    assert!(!keys.contains(&"lineWidth".to_string()));
    let errors = load_one("lineWidth", "80").unwrap_err();
    assert_eq!(
        errors,
        [ConfigErrorKind::UnknownKey("lineWidth".to_string())]
    );
}

#[test]
fn key_of_record_unknown_to_schema_of_overrides_fails_to_load() {
    let text = schema();
    let override_keys: Vec<_> = override_properties(&text)
        .into_iter()
        .map(|(key, _)| key)
        .collect();
    let same_for_every_file = properties(&text)
        .into_iter()
        .filter(|(key, _)| !override_keys.contains(key));
    for (key, property) in same_for_every_file {
        let value = &allowed_values(&key, &property)[0];
        let errors = load_in_override(&key, value).unwrap_err();
        assert!(
            matches!(&errors[..], [ConfigErrorKind::NotPerFile(rejected)] if *rejected == key),
            "{key}: {errors:?}"
        );
    }
    let errors = load_in_override("lineWidth", "80").unwrap_err();
    assert_eq!(
        errors,
        [ConfigErrorKind::UnknownKey("lineWidth".to_string())]
    );
}

#[test]
fn schema_is_as_snapshot() {
    insta::assert_snapshot!(schema());
}

#[test]
fn schema_ends_with_one_line_break() {
    // The snapshot cannot pin this, since insta trims the line breaks at the end of one.
    let text = schema();
    assert!(text.ends_with("}\n"));
    assert!(!text.ends_with("\n\n"));
}
