//! Writes the JSON Schema of a config file.

use std::fmt::{self, Write};

use crate::settings::{Kind, SETTINGS, Setting};

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
    format!("{root}\n")
}

fn property(setting: &Setting) -> Json {
    let description = ("description", Json::String(setting.description));
    match setting.kind {
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
        Kind::Boolean {
            ..
        } => Json::object([description, ("type", Json::String("boolean"))]),
        Kind::Patterns {
            ..
        } => Json::object([
            description,
            ("type", Json::String("array")),
            ("items", Json::object([("type", Json::String("string"))])),
        ]),
        Kind::String => Json::object([description, ("type", Json::String("string"))]),
    }
}

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

    /// Writes `self` with its lines after the first indented `depth` levels.
    /// An array stays on one line, since the schema holds no array but short ones of strings.
    fn write(&self, f: &mut fmt::Formatter<'_>, depth: usize) -> fmt::Result {
        let indent = |depth| "  ".repeat(depth);
        match self {
            Json::Object(fields) if fields.is_empty() => f.write_str("{}"),
            Json::Object(fields) => {
                f.write_str("{\n")?;
                for (index, (key, value)) in fields.iter().enumerate() {
                    let separator = if index + 1 == fields.len() { "" } else { "," };
                    write!(f, "{}", indent(depth + 1))?;
                    write_string(f, key)?;
                    f.write_str(": ")?;
                    value.write(f, depth + 1)?;
                    writeln!(f, "{separator}")?;
                }
                write!(f, "{}}}", indent(depth))
            },
            Json::Array(items) => {
                f.write_char('[')?;
                for (index, item) in items.iter().enumerate() {
                    if index > 0 {
                        f.write_str(", ")?;
                    }
                    item.write(f, depth)?;
                }
                f.write_char(']')
            },
            Json::String(text) => write_string(f, text),
            Json::Integer(integer) => write!(f, "{integer}"),
            Json::Bool(boolean) => write!(f, "{boolean}"),
        }
    }
}

impl fmt::Display for Json {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.write(f, 0)
    }
}

/// Keys, choices, and descriptions have no control characters, so only `"` and `\` need escapes.
fn write_string(f: &mut fmt::Formatter<'_>, text: &str) -> fmt::Result {
    write!(f, "\"{}\"", text.replace('\\', "\\\\").replace('"', "\\\""))
}

#[cfg(test)]
mod tests {
    use super::Json;

    #[test]
    fn object_puts_each_field_on_its_own_line() {
        let json = Json::object([
            ("a", Json::Integer(1)),
            ("b", Json::object([("c", Json::Bool(true))])),
            ("d", Json::object([])),
        ]);
        assert_eq!(
            json.to_string(),
            "{\n  \"a\": 1,\n  \"b\": {\n    \"c\": true\n  },\n  \"d\": {}\n}"
        );
    }

    #[test]
    fn array_stays_on_one_line() {
        let json = Json::Array(vec![Json::String("a"), Json::Integer(-1)]);
        assert_eq!(json.to_string(), r#"["a", -1]"#);
    }

    #[test]
    fn string_escapes_quote_and_backslash() {
        assert_eq!(Json::String(r#"a"b\c"#).to_string(), r#""a\"b\\c""#);
    }
}
