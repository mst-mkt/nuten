//! Reads NUON, the values nu writes as nu code.
//!
//! Nu reads any value its parser gives without running code.
//! Nuten reads only the forms it knows nu to read the same way, and reports any other.

use nuten_syntax::{NodeOrToken, SyntaxElement, SyntaxKind, SyntaxNode, SyntaxToken, TextRange};

use crate::{
    ConfigError, ConfigErrorKind, error,
    value::{self, Field, Value, ValueKind},
};

pub(crate) fn read(text: &str) -> Result<Value, Vec<ConfigError>> {
    let parsed = nuten_parser::parse(text);
    if !parsed.errors().is_empty() {
        return Err(parsed
            .errors()
            .iter()
            .map(|error| ConfigError {
                kind: ConfigErrorKind::Nuon(error.kind),
                range: error.range,
            })
            .collect());
    }
    // A pipeline of a lone `|` has no element, and nu reads no value from it.
    let elements: Vec<_> = parsed
        .syntax()
        .children()
        .flat_map(|pipeline| pipeline.children())
        .collect();
    match &elements[..] {
        [] => Ok(value::absent()),
        [element] => element_value(element),
        [_, extra, ..] => Err(error(ConfigErrorKind::ExtraValue, extra.text_range())),
    }
}

/// The value `element` is, bare of any command or redirection.
fn element_value(element: &SyntaxNode) -> Result<Value, Vec<ConfigError>> {
    match &significant(element)[..] {
        [value] => self::value(value),
        _ => Err(unreadable(element.text_range())),
    }
}

fn value(element: &SyntaxElement) -> Result<Value, Vec<ConfigError>> {
    match element {
        NodeOrToken::Token(token) => scalar(token)
            .map(|kind| Value {
                kind,
                range: token.text_range(),
            })
            .ok_or_else(|| unreadable(element.text_range())),
        NodeOrToken::Node(node) if node.kind() == SyntaxKind::Record => record(node),
        NodeOrToken::Node(node) if node.kind() == SyntaxKind::List => list(node),
        NodeOrToken::Node(_) => Err(unreadable(element.text_range())),
    }
}

fn list(node: &SyntaxNode) -> Result<Value, Vec<ConfigError>> {
    let values = value::all(items(node).iter().map(value))?;
    Ok(Value {
        kind: ValueKind::List(values),
        range: node.text_range(),
    })
}

fn record(node: &SyntaxNode) -> Result<Value, Vec<ConfigError>> {
    let fields = value::all(items(node).iter().map(|item| match item {
        NodeOrToken::Node(field) if field.kind() == SyntaxKind::RecordField => self::field(field),
        _ => Err(unreadable(item.text_range())),
    }))?;
    value::record(fields, node.text_range())
}

fn field(node: &SyntaxNode) -> Result<Field, Vec<ConfigError>> {
    let parts = significant(node);
    let [NodeOrToken::Token(key), NodeOrToken::Token(colon), value] = &parts[..] else {
        return Err(unreadable(node.text_range()));
    };
    if colon.kind() != SyntaxKind::Colon {
        return Err(unreadable(node.text_range()));
    }
    // As keys, nu reads `true` as an error and `1` as a string, unlike as values.
    // `scalar` gives neither as a string, so both are left unread.
    let Some(ValueKind::String(key_text)) = scalar(key) else {
        return Err(unreadable(key.text_range()));
    };
    Ok(Field {
        key: key_text,
        key_range: key.text_range(),
        value: self::value(value)?,
    })
}

/// What a lone token reads as, or `None` for a form nuten does not read.
fn scalar(token: &SyntaxToken) -> Option<ValueKind> {
    let text = token.text();
    match token.kind() {
        SyntaxKind::Word => word(text),
        SyntaxKind::String => quoted(text).map(|content| ValueKind::String(content.to_string())),
        _ => None,
    }
}

fn word(text: &str) -> Option<ValueKind> {
    match text {
        "true" => Some(ValueKind::Bool(true)),
        "false" => Some(ValueKind::Bool(false)),
        "null" => Some(ValueKind::Null),
        // Nu reads one too large for an `i64` as a float, which nuten leaves unread.
        _ if is_decimal_integer(text) => text.parse().ok().map(ValueKind::Int),
        _ => is_bare_string(text).then(|| ValueKind::String(text.to_string())),
    }
}

/// Nuten leaves unread the `_` among digits and the radix prefixes that nu reads as well.
fn is_decimal_integer(text: &str) -> bool {
    let digits = text.strip_prefix(['+', '-']).unwrap_or(text);
    !digits.is_empty() && digits.bytes().all(|byte| byte.is_ascii_digit())
}

/// A leading `_` or digit may make a number, as `_1` does.
/// Nu reads `inf`, `nan`, and the like as floats, whatever their letter case.
/// Nu drops every `_` before it reads a float, so `n_an` is one too.
fn is_bare_string(text: &str) -> bool {
    text.chars()
        .next()
        .is_some_and(|first| first.is_ascii_alphabetic())
        && text
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || matches!(character, '_' | '-'))
        && text.replace('_', "").parse::<f64>().is_err()
}

/// Nu's escapes are left unread rather than decoded again here.
/// A wrong decoding would change a setting unseen.
fn quoted(text: &str) -> Option<&str> {
    let content = &text[1..text.len() - 1];
    match text.as_bytes()[0] {
        b'"' => (!content.contains('\\')).then_some(content),
        b'\'' | b'`' => Some(content),
        _ => None,
    }
}

fn significant(node: &SyntaxNode) -> Vec<SyntaxElement> {
    node.children_with_tokens()
        .filter(|child| !child.kind().is_trivia())
        .collect()
}

fn items(node: &SyntaxNode) -> Vec<SyntaxElement> {
    significant(node)
        .into_iter()
        .filter(|child| {
            !matches!(
                child.kind(),
                SyntaxKind::LBrace
                    | SyntaxKind::RBrace
                    | SyntaxKind::LBracket
                    | SyntaxKind::RBracket
                    | SyntaxKind::Comma
            )
        })
        .collect()
}

fn unreadable(range: TextRange) -> Vec<ConfigError> {
    error(ConfigErrorKind::Unreadable, range)
}

#[cfg(test)]
mod tests {
    use crate::{
        Config, ConfigErrorKind, Format, FormatConfig, IndentStyle, LineEnding, load, read,
        test_support,
        value::{Value, ValueKind},
    };

    fn errors(text: &str) -> Vec<(String, &str)> {
        test_support::errors(text, Format::Nuon)
    }

    fn unreadable() -> String {
        ConfigErrorKind::Unreadable.to_string()
    }

    /// What the one field of the record `text` holds.
    fn field_value(text: &str) -> ValueKind {
        let Ok(Value {
            kind: ValueKind::Record(fields),
            ..
        }) = read(text, Format::Nuon)
        else {
            panic!("{text} is no record");
        };
        fields.into_iter().next().unwrap().value.kind
    }

    #[test]
    fn record_of_any_quoting_and_spacing_gives_settings() {
        let text = concat!(
            "# nuten\n",
            "{\n",
            "    line_width: 80 # c\n",
            "    'indent_style': tab,\n",
            "    \"indent_width\": 2, `line_ending`: \"crlf\"\n",
            "    respect_gitignore: false\n",
            "}\n",
        );
        let expected = Config {
            format: FormatConfig {
                line_width: 80,
                indent_style: IndentStyle::Tab,
                indent_width: 2,
                line_ending: LineEnding::CrLf,
            },
            respect_gitignore: false,
            ..Config::default()
        };
        assert_eq!(load(text, Format::Nuon), Ok(expected));
    }

    #[test]
    fn empty_record_gives_defaults() {
        assert_eq!(load("{}", Format::Nuon), Ok(Config::default()));
    }

    #[test]
    fn field_without_space_after_colon_is_read() {
        assert_eq!(
            load("{line_width:80}", Format::Nuon)
                .unwrap()
                .format
                .line_width,
            80
        );
    }

    #[test]
    fn field_split_across_lines_is_read() {
        let text = "{\n  line_width:\n  # c\n  80,\n  \"indent_width\" : 2,\n}";
        let config = load(text, Format::Nuon).unwrap();
        assert_eq!(
            (config.format.line_width, config.format.indent_width),
            (80, 2)
        );
    }

    #[test]
    fn crlf_line_endings_are_read() {
        let text = "{\r\n  line_width: 80 # c\r\n}\r\n";
        assert_eq!(load(text, Format::Nuon).unwrap().format.line_width, 80);
    }

    #[test]
    fn schema_key_is_read_when_quoted() {
        let text = r#"{"$schema": "./nuten.schema.json", line_width: 80}"#;
        assert_eq!(load(text, Format::Nuon).unwrap().format.line_width, 80);
    }

    #[test]
    fn decimal_word_is_int() {
        for (text, int) in [
            ("{a: 5}", 5),
            ("{a: -5}", -5),
            ("{a: +5}", 5),
            ("{a: 05}", 5),
            ("{a: -0}", 0),
            ("{a: 9223372036854775807}", i64::MAX),
            ("{a: -9223372036854775808}", i64::MIN),
        ] {
            assert_eq!(field_value(text), ValueKind::Int(int), "{text}");
        }
    }

    #[test]
    fn keyword_word_is_bool_or_null() {
        assert_eq!(field_value("{a: true}"), ValueKind::Bool(true));
        assert_eq!(field_value("{a: false}"), ValueKind::Bool(false));
        assert_eq!(field_value("{a: null}"), ValueKind::Null);
    }

    #[test]
    fn bare_word_is_string() {
        for text in [
            "tab", "crlf", "TRUE", "Null", "a-b", "a_b-c", "x-", "e5", "in", "if", "not", "e_",
            "in_", "i_n", "inf_x", "inf-", "na-n",
        ] {
            let record = format!("{{a: {text}}}");
            assert_eq!(
                field_value(&record),
                ValueKind::String(text.to_string()),
                "{text}"
            );
        }
    }

    #[test]
    fn quoted_string_without_escapes_is_its_content() {
        for (text, content) in [
            (r#"{a: "x y"}"#, "x y"),
            (r"{a: 'x\ty'}", r"x\ty"),
            ("{a: `x y`}", "x y"),
            (r#"{a: ""}"#, ""),
            ("{a: \"x\r\ny\"}", "x\r\ny"),
        ] {
            assert_eq!(
                field_value(text),
                ValueKind::String(content.to_string()),
                "{text}"
            );
        }
    }

    #[test]
    fn list_holds_its_items() {
        let ValueKind::List(items) = field_value("{a: [lf, 1\n # c\n 'x',]}") else {
            panic!("no list");
        };
        let kinds: Vec<_> = items.into_iter().map(|item| item.kind).collect();
        assert_eq!(
            kinds,
            [
                ValueKind::String("lf".to_string()),
                ValueKind::Int(1),
                ValueKind::String("x".to_string())
            ]
        );
    }

    #[test]
    fn value_nu_may_read_otherwise_is_unreadable() {
        for value in [
            "0x64",
            "0b11",
            "1_000",
            "_1",
            "1.5",
            "1e2",
            "inf",
            "NaN",
            "-inf",
            "inf_",
            "i_nf",
            "n_an",
            "NaN_",
            "infinity",
            "1kb",
            "1sec",
            "2024-01-01",
            "0.2.0",
            "9223372036854775808",
            "r#'x'#",
            r#"$"x""#,
            "(1)",
            "$x",
            r#"x"y""#,
            "é",
            "1..2",
            "{|| 1}",
            "[[a]; [1]]",
        ] {
            let text = format!("{{a: {value}}}");
            assert_eq!(errors(&text), [(unreadable(), value)], "{text}");
        }
    }

    #[test]
    fn quoted_string_with_any_escape_is_unreadable() {
        for value in [r#""\n""#, r#""\t""#, r#""\"""#, r#""\\""#, r#""t\u{61}b""#] {
            let text = format!("{{a: {value}}}");
            assert_eq!(errors(&text), [(unreadable(), value)], "{text}");
        }
    }

    #[test]
    fn key_nu_may_read_otherwise_is_unreadable() {
        for key in ["true", "null", "1", "inf", "$schema", r#""a\tb""#] {
            let text = format!("{{{key}: 1}}");
            assert_eq!(errors(&text), [(unreadable(), key)], "{text}");
        }
    }

    #[test]
    fn irregular_entry_is_unreadable() {
        for (text, at) in [
            ("{a: 1 b}", &["b"][..]),
            ("{a: 1; b: 2}", &["; b:", "2"]),
            ("{...$x}", &["...$x"]),
            ("{a: 1} o> x", &["{a: 1} o> x"]),
        ] {
            let expected: Vec<_> = at.iter().map(|at| (unreadable(), *at)).collect();
            assert_eq!(errors(text), expected, "{text}");
        }
    }

    #[test]
    fn value_after_first_is_reported_at_second() {
        for (text, at) in [
            ("{}\n{}", "{}"),
            ("{}; 1; 2", "1"),
            ("{} | length | x", "length"),
        ] {
            assert_eq!(
                errors(text),
                [("value after the first; NUON holds one".to_string(), at)],
                "{text}"
            );
        }
    }

    #[test]
    fn lone_pipe_holds_no_value() {
        for (text, expected) in [
            ("|\n", ValueKind::Null),
            ("| # c\n", ValueKind::Null),
            (";\n|\n", ValueKind::Null),
            ("|\n \n{}", ValueKind::Record(Vec::new())),
            ("{};\n|\n", ValueKind::Record(Vec::new())),
            ("  [];|\n", ValueKind::List(Vec::new())),
        ] {
            assert_eq!(
                read(text, Format::Nuon).map(|value| value.kind),
                Ok(expected),
                "{text:?}"
            );
        }
    }

    #[test]
    fn file_without_value_is_null() {
        for text in ["", "# c\n", "\n\n"] {
            assert_eq!(
                read(text, Format::Nuon).map(|value| value.kind),
                Ok(ValueKind::Null),
                "{text:?}"
            );
        }
    }

    #[test]
    fn every_syntax_error_is_reported_as_parser_reports_it() {
        let text = "{a: ) b: \"x";
        let reported = load(text, Format::Nuon).unwrap_err();
        let parsed = nuten_parser::parse(text);
        let expected: Vec<_> = parsed
            .errors()
            .iter()
            .map(|error| (ConfigErrorKind::Nuon(error.kind), error.range))
            .collect();
        assert!(expected.len() > 1);
        let got: Vec<_> = reported
            .into_iter()
            .map(|error| (error.kind, error.range))
            .collect();
        assert_eq!(got, expected);
    }

    #[test]
    fn duplicate_key_is_reported_at_later_key() {
        assert_eq!(
            errors("{a: 1, 'a': 2}"),
            [("duplicate key `a`".to_string(), "'a'")]
        );
    }

    #[test]
    fn file_of_no_record_is_reported() {
        for (text, found) in [
            ("[1]", "a list"),
            ("1", "an integer"),
            ("'tab'", "a string"),
        ] {
            assert_eq!(
                errors(text),
                [(format!("config is {found}, not a record"), text)]
            );
        }
    }

    #[test]
    fn byte_order_mark_is_unreadable() {
        let text = "\u{feff}{}";
        assert_eq!(errors(text), [(unreadable(), text)]);
    }
}
