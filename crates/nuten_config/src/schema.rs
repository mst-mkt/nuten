use serde::{Serialize, Serializer};

use crate::settings::{Kind, PICKERS, SETTINGS, Scope, Setting};

pub fn schema() -> String {
    let properties = SETTINGS
        .iter()
        .map(|setting| (setting.key, property(setting)));
    let root = Json::object([
        (
            "$schema",
            Json::String("https://json-schema.org/draft/2020-12/schema"),
        ),
        ("title", Json::String("nuten")),
        (
            "description",
            Json::String("Settings of nuten, a formatter for Nushell."),
        ),
        ("type", Json::String("object")),
        ("properties", Json::object(properties)),
        ("additionalProperties", Json::Bool(false)),
    ]);
    serde_json::to_string_pretty(&root).expect("the schema has only string keys") + "\n"
}

fn property(setting: &Setting) -> Json {
    let description = ("description", Json::String(setting.description));
    match &setting.scope {
        Scope::File(kind) => property_of(description, kind),
        Scope::Whole(kind) => property_of(description, kind),
    }
}

fn property_of<C>(description: (&'static str, Json), kind: &Kind<C>) -> Json {
    match *kind {
        // The schema lets `80.0` pass as an integer, which nuten rejects as a float.
        // No keyword tells the two apart.
        Kind::Integer {
            min,
            max,
            ..
        } => Json::object([
            description,
            ("type", Json::String("integer")),
            ("minimum", Json::Integer(min)),
            ("maximum", Json::Integer(max)),
        ]),
        Kind::Choice(choices) => Json::object([
            description,
            (
                "enum",
                Json::Array(
                    choices
                        .iter()
                        .map(|choice| Json::String(choice.name))
                        .collect(),
                ),
            ),
        ]),
        Kind::Boolean(_) => Json::object([description, ("type", Json::String("boolean"))]),
        Kind::Patterns(_) => patterns(description),
        Kind::Overrides(_) => Json::object([
            description,
            ("type", Json::String("array")),
            ("items", override_record()),
        ]),
        Kind::String => Json::object([description, ("type", Json::String("string"))]),
    }
}

fn patterns(description: (&'static str, Json)) -> Json {
    Json::object([
        description,
        ("type", Json::String("array")),
        ("items", Json::object([("type", Json::String("string"))])),
    ])
}

fn override_record() -> Json {
    let pickers = PICKERS
        .map(|(key, description)| (key, patterns(("description", Json::String(description)))));
    let settings = SETTINGS
        .iter()
        .filter(|setting| matches!(setting.scope, Scope::File(_)))
        .map(|setting| (setting.key, property(setting)));
    Json::object([
        ("type", Json::String("object")),
        (
            "properties",
            Json::object(pickers.into_iter().chain(settings)),
        ),
        ("required", Json::Array(vec![Json::String("include")])),
        ("additionalProperties", Json::Bool(false)),
    ])
}

/// Not `serde_json::Value`, whose map sorts the keys unless `preserve_order` brings in `indexmap`.
#[derive(Debug)]
enum Json {
    Object(Vec<(&'static str, Json)>),
    Array(Vec<Json>),
    String(&'static str),
    Integer(i64),
    Bool(bool),
}

impl Json {
    fn object(fields: impl IntoIterator<Item = (&'static str, Json)>) -> Json {
        Json::Object(fields.into_iter().collect())
    }
}

impl Serialize for Json {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            Json::Object(fields) => {
                serializer.collect_map(fields.iter().map(|(key, value)| (key, value)))
            },
            Json::Array(items) => serializer.collect_seq(items),
            Json::String(text) => serializer.serialize_str(text),
            Json::Integer(integer) => serializer.serialize_i64(*integer),
            Json::Bool(boolean) => serializer.serialize_bool(*boolean),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Json;

    #[test]
    fn object_keeps_order_of_its_fields() {
        let json = Json::object([("b", Json::Integer(1)), ("a", Json::Bool(true))]);
        assert_eq!(serde_json::to_string(&json).unwrap(), r#"{"b":1,"a":true}"#);
    }

    #[test]
    fn string_escapes_quote_backslash_and_control_characters() {
        let json = Json::String("a\"b\\c\nd\u{1}");
        assert_eq!(
            serde_json::to_string(&json).unwrap(),
            r#""a\"b\\c\nd\u0001""#
        );
    }
}
