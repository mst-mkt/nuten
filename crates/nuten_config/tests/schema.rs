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

fn properties(text: &str) -> Vec<(String, JsonObject<'_>)> {
    let Some(JsonValue::Object(properties)) = root(text).take("properties") else {
        panic!("the schema has no properties");
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

fn integer_edge(property: &JsonObject<'_>, edge: &str) -> i64 {
    property
        .get_number(edge)
        .expect("an integer has both edges")
        .parse()
        .unwrap()
}

fn load_one(key: &str, value: &str) -> Result<Config, Vec<ConfigErrorKind>> {
    load(&format!("{{\"{key}\": {value}}}"), Format::Json)
        .map_err(|errors| errors.into_iter().map(|error| error.kind).collect())
}

#[test]
fn schema_allowed_value_loads() {
    for (key, property) in properties(&schema()) {
        let values: Vec<String> = match property.get_string("type").map(AsRef::as_ref) {
            Some("integer") => ["minimum", "maximum"]
                .map(|edge| integer_edge(&property, edge).to_string())
                .into(),
            Some("string") => vec!["\"x\"".to_string()],
            Some("boolean") => vec!["true".to_string(), "false".to_string()],
            _ => property
                .get_array("enum")
                .expect("a property has a type or choices")
                .iter()
                .map(|choice| match choice {
                    JsonValue::String(choice) => format!("\"{choice}\""),
                    _ => panic!("{key} has a choice of no string"),
                })
                .collect(),
        };
        for value in values {
            assert!(load_one(&key, &value).is_ok(), "{key}: {value}");
        }
    }
}

#[test]
fn schema_rejected_value_fails_to_load() {
    for (key, property) in properties(&schema()) {
        match property.get_string("type").map(AsRef::as_ref) {
            Some("integer") => {
                let below = integer_edge(&property, "minimum") - 1;
                let above = integer_edge(&property, "maximum") + 1;
                for value in [below, above] {
                    let errors = load_one(&key, &value.to_string()).unwrap_err();
                    assert!(
                        matches!(errors[..], [ConfigErrorKind::OutOfRange { .. }]),
                        "{key}: {value}"
                    );
                }
            },
            Some("string") => {
                let errors = load_one(&key, "1").unwrap_err();
                assert!(
                    matches!(errors[..], [ConfigErrorKind::WrongType { .. }]),
                    "{key}"
                );
            },
            Some("boolean") => {
                let errors = load_one(&key, "\"true\"").unwrap_err();
                assert!(
                    matches!(errors[..], [ConfigErrorKind::WrongType { .. }]),
                    "{key}"
                );
            },
            _ => {
                let errors = load_one(&key, "\"none of the choices\"").unwrap_err();
                assert!(
                    matches!(errors[..], [ConfigErrorKind::UnknownChoice { .. }]),
                    "{key}"
                );
            },
        }
    }
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
