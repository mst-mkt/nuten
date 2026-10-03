use jsonc_parser::{CollectOptions, ParseOptions, ast, common::Range, parse_to_ast};
use nuten_syntax::{TextRange, TextSize};

use crate::{
    ConfigError, ConfigErrorKind, error,
    value::{self, Field, Value, ValueKind},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Dialect {
    Json,
    Jsonc,
}

impl Dialect {
    /// The parser takes more than either dialect by default, such as single-quoted strings.
    /// It still takes control characters in strings, and spaces JSON does not know between tokens.
    /// Those are left as the parser reads them, since none changes what a value is.
    fn options(self) -> ParseOptions {
        let jsonc = self == Dialect::Jsonc;
        ParseOptions {
            allow_comments: jsonc,
            allow_trailing_commas: jsonc,
            allow_loose_object_property_names: false,
            allow_missing_commas: false,
            allow_single_quoted_strings: false,
            allow_hexadecimal_numbers: false,
            allow_unary_plus_numbers: false,
        }
    }
}

pub(crate) fn read(text: &str, dialect: Dialect) -> Result<Value, Vec<ConfigError>> {
    let parsed = parse_to_ast(text, &CollectOptions::default(), &dialect.options()).map_err(
        |parse_error| {
            error(
                ConfigErrorKind::Json(parse_error.kind().to_string()),
                text_range(parse_error.range()),
            )
        },
    )?;
    parsed.value.map_or_else(|| Ok(value::absent()), value)
}

fn value(json: ast::Value<'_>) -> Result<Value, Vec<ConfigError>> {
    let (kind, range) = match json {
        ast::Value::Object(object) => {
            let fields = value::all(object.properties.into_iter().map(field))?;
            return value::record(fields, text_range(object.range));
        },
        ast::Value::Array(array) => (
            ValueKind::List(value::all(array.elements.into_iter().map(value))?),
            array.range,
        ),
        ast::Value::StringLit(string) => {
            (ValueKind::String(string.value.into_owned()), string.range)
        },
        ast::Value::NumberLit(number) => (self::number(number.value), number.range),
        ast::Value::BooleanLit(boolean) => (ValueKind::Bool(boolean.value), boolean.range),
        ast::Value::NullKeyword(null) => (ValueKind::Null, null.range),
    };
    Ok(Value {
        kind,
        range: text_range(range),
    })
}

fn field(property: ast::ObjectProp<'_>) -> Result<Field, Vec<ConfigError>> {
    let key_range = match &property.name {
        ast::ObjectPropName::String(string) => string.range,
        ast::ObjectPropName::Word(word) => word.range,
    };
    Ok(Field {
        key: property.name.into_string(),
        key_range: text_range(key_range),
        value: value(property.value)?,
    })
}

fn number(text: &str) -> ValueKind {
    match text.parse() {
        Ok(int) => ValueKind::Int(int),
        Err(_) => ValueKind::Float(text.parse().expect("a JSON number reads as an f64")),
    }
}

fn text_range(range: Range) -> TextRange {
    let offset =
        |offset: usize| TextSize::try_from(offset).expect("a config file is shorter than 4 GiB");
    TextRange::new(offset(range.start), offset(range.end))
}

#[cfg(test)]
mod tests {
    use super::number;
    use crate::{
        ConfigError, ConfigErrorKind, Format, FormatConfig, IndentStyle, load,
        test_support::errors, value::ValueKind,
    };

    fn is_json_error(reported: &[ConfigError]) -> bool {
        matches!(
            reported,
            [ConfigError {
                kind: ConfigErrorKind::Json(_),
                ..
            }]
        )
    }

    #[test]
    fn number_without_fraction_or_exponent_is_int() {
        assert_eq!(number("100"), ValueKind::Int(100));
        assert_eq!(number("-0"), ValueKind::Int(0));
    }

    #[test]
    fn number_with_fraction_or_exponent_is_float() {
        assert_eq!(number("100.0"), ValueKind::Float(100.0));
        assert_eq!(number("1e2"), ValueKind::Float(100.0));
    }

    #[test]
    fn integer_too_large_for_i64_is_float() {
        assert_eq!(
            number("9223372036854775808"),
            ValueKind::Float(9_223_372_036_854_775_808.0)
        );
    }

    #[test]
    fn json_rejects_comments_and_trailing_commas() {
        for text in [
            "// c\n{}",
            "{} /* c */",
            r#"{"line_width": 80,}"#,
            "{\"a\": [1,]}",
        ] {
            assert!(
                is_json_error(&load(text, Format::Json).unwrap_err()),
                "{text}"
            );
        }
    }

    #[test]
    fn jsonc_takes_comments_and_trailing_commas() {
        let text = "// c\n{\n  \"line_width\": 80, /* c */\n  \"$schema\": [1,],\n}\n";
        assert_eq!(
            errors(text, Format::Jsonc),
            [("`$schema` takes a string, not a list".to_string(), "[1,]")]
        );
        let text = "/* c */ {\"line_width\": 80,} // c\n";
        assert_eq!(load(text, Format::Jsonc).unwrap().format.line_width, 80);
    }

    #[test]
    fn jsonc_of_comments_alone_is_null() {
        assert_eq!(
            errors("// c\n/* c */\n", Format::Jsonc),
            [("config is null, not a record".to_string(), "")]
        );
    }

    #[test]
    fn json_and_jsonc_reject_what_only_jsonc_parser_takes() {
        for text in [
            "{line_width: 80}",
            r#"{"line_width": 80 "indent_width": 2}"#,
            "{'line_width': 80}",
            r#"{"line_width": 0x50}"#,
            r#"{"line_width": +80}"#,
        ] {
            for format in [Format::Json, Format::Jsonc] {
                assert!(
                    is_json_error(&load(text, format).unwrap_err()),
                    "{text} {format:?}"
                );
            }
        }
    }

    #[test]
    fn syntax_error_is_reported_where_parser_says() {
        let text = "{\"line_width\": }";
        let reported = load(text, Format::Json).unwrap_err();
        assert!(is_json_error(&reported));
        assert_eq!(&text[reported[0].range], "}");
    }

    #[test]
    fn string_escapes_are_decoded() {
        let text = r#"{"indent_style": "t\u0061b", "l\u0069ne_width": 80}"#;
        assert_eq!(
            load(text, Format::Json).unwrap().format,
            FormatConfig {
                indent_style: IndentStyle::Tab,
                line_width: 80,
                ..FormatConfig::default()
            }
        );
    }

    #[test]
    fn crlf_line_endings_are_read() {
        let text = "{\r\n  \"line_width\": 80 // c\r\n}\r\n";
        assert_eq!(load(text, Format::Jsonc).unwrap().format.line_width, 80);
        let text = "{\r\n  \"line_width\": 80\r\n}\r\n";
        assert_eq!(load(text, Format::Json).unwrap().format.line_width, 80);
    }

    #[test]
    fn byte_order_mark_is_rejected_at_start() {
        for format in [Format::Json, Format::Jsonc] {
            let text = "\u{feff}{}";
            let reported = load(text, format).unwrap_err();
            assert!(is_json_error(&reported), "{format:?}");
            assert_eq!(&text[reported[0].range], "\u{feff}", "{format:?}");
        }
    }
}
