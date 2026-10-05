use nuten_syntax::{NodeOrToken, SyntaxElement, SyntaxKind, SyntaxNode, SyntaxToken, TextRange};

use crate::{
    ConfigError, ConfigErrorKind, error, escape,
    value::{self, Field, Value, ValueKind},
    word::{self, Place, Word},
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

fn element_value(element: &SyntaxNode) -> Result<Value, Vec<ConfigError>> {
    match &significant(element)[..] {
        [value] => self::value(value, Place::Item),
        _ => Err(rejected(element.text_range())),
    }
}

fn value(element: &SyntaxElement, place: Place) -> Result<Value, Vec<ConfigError>> {
    let range = element.text_range();
    let kind = match element {
        NodeOrToken::Token(token) => scalar(token, place),
        NodeOrToken::Node(node) => match node.kind() {
            SyntaxKind::Record => return record(node),
            SyntaxKind::List => return list(node),
            SyntaxKind::Table => return table(node),
            SyntaxKind::Range => has_bare_bounds(node).then_some(ValueKind::Range),
            SyntaxKind::Binary => Some(ValueKind::Binary),
            // A cell path literal starts with a lone `$`; any other starts with a variable.
            SyntaxKind::CellPath => node
                .first_token()
                .is_some_and(|head| head.text() == "$")
                .then_some(ValueKind::CellPath),
            _ => None,
        },
    };
    kind.map(|kind| Value {
        kind,
        range,
    })
    .ok_or_else(|| rejected(range))
}

/// Nu rejects a range with a variable or a subexpression as a bound.
fn has_bare_bounds(node: &SyntaxNode) -> bool {
    node.children_with_tokens()
        .filter(|child| !child.kind().is_trivia())
        .all(|child| match child {
            NodeOrToken::Token(token) => {
                token.kind() == SyntaxKind::RangeOperator
                    || (token.kind() == SyntaxKind::Word && !token.text().starts_with('$'))
            },
            NodeOrToken::Node(_) => false,
        })
}

fn table(node: &SyntaxNode) -> Result<Value, Vec<ConfigError>> {
    let parts: Vec<_> = items(node)
        .into_iter()
        .filter(|part| part.kind() != SyntaxKind::Semicolon)
        .collect();
    let [NodeOrToken::Node(header), rows @ ..] = &parts[..] else {
        return Err(rejected(node.text_range()));
    };
    let columns = value::all(items(header).iter().map(column))?;
    let duplicates: Vec<_> = columns
        .iter()
        .enumerate()
        .filter(|(index, column)| {
            columns[..*index]
                .iter()
                .any(|before| before.name == column.name)
        })
        .flat_map(|(_, column)| {
            error(
                ConfigErrorKind::DuplicateKey(column.name.clone()),
                column.range,
            )
        })
        .collect();
    if !duplicates.is_empty() {
        return Err(duplicates);
    }
    let rows = value::all(rows.iter().map(|row| self::row(row, &columns)))?;
    Ok(Value {
        kind: ValueKind::List(rows),
        range: node.text_range(),
    })
}

struct Column {
    name: String,
    range: TextRange,
}

/// Nu reads a column as any value, then takes only a string.
/// A raw string does not count as one there.
fn column(element: &SyntaxElement) -> Result<Column, Vec<ConfigError>> {
    let range = element.text_range();
    match element {
        NodeOrToken::Token(token) if token.kind() != SyntaxKind::RawString => {
            match scalar(token, Place::Item) {
                Some(ValueKind::String(name)) => Ok(Column {
                    name,
                    range,
                }),
                _ => Err(rejected(range)),
            }
        },
        _ => Err(rejected(range)),
    }
}

fn row(element: &SyntaxElement, columns: &[Column]) -> Result<Value, Vec<ConfigError>> {
    let range = element.text_range();
    let cells = match element {
        NodeOrToken::Node(node) if node.kind() == SyntaxKind::List => items(node),
        _ => return Err(rejected(range)),
    };
    if cells.len() != columns.len() {
        return Err(rejected(range));
    }
    let fields = value::all(columns.iter().zip(&cells).map(|(column, cell)| {
        Ok(Field {
            key: column.name.clone(),
            key_range: column.range,
            value: value(cell, Place::Item)?,
        })
    }))?;
    Ok(Value {
        kind: ValueKind::Record(fields),
        range,
    })
}

fn list(node: &SyntaxNode) -> Result<Value, Vec<ConfigError>> {
    let values = value::all(items(node).iter().map(|item| value(item, Place::Item)))?;
    Ok(Value {
        kind: ValueKind::List(values),
        range: node.text_range(),
    })
}

fn record(node: &SyntaxNode) -> Result<Value, Vec<ConfigError>> {
    let fields = value::all(items(node).iter().map(|item| match item {
        NodeOrToken::Node(field) if field.kind() == SyntaxKind::RecordField => self::field(field),
        _ => Err(rejected(item.text_range())),
    }))?;
    value::record(fields, node.text_range())
}

fn field(node: &SyntaxNode) -> Result<Field, Vec<ConfigError>> {
    let parts = significant(node);
    let [NodeOrToken::Token(key), NodeOrToken::Token(colon), value] = &parts[..] else {
        return Err(rejected(node.text_range()));
    };
    if colon.kind() != SyntaxKind::Colon {
        return Err(rejected(node.text_range()));
    }
    let Some(key_text) = self::key(key) else {
        return Err(rejected(key.text_range()));
    };
    Ok(Field {
        key: key_text,
        key_range: key.text_range(),
        value: self::value(value, Place::RecordValue)?,
    })
}

/// Nu reads a key as a string whatever it looks like, but for `true`, `false`, and `null`.
fn key(token: &SyntaxToken) -> Option<String> {
    let text = token.text();
    match token.kind() {
        SyntaxKind::Word => match word::read(text, Place::Item) {
            Word::Value(ValueKind::String(key)) => Some(key),
            Word::Value(ValueKind::Bool(_) | ValueKind::Null) | Word::Rejected => None,
            Word::Value(_) => Some(text.to_string()),
        },
        SyntaxKind::String => quoted(text),
        _ => None,
    }
}

fn scalar(token: &SyntaxToken, place: Place) -> Option<ValueKind> {
    let text = token.text();
    match token.kind() {
        SyntaxKind::Word => match word::read(text, place) {
            Word::Value(kind) => Some(kind),
            Word::Rejected => None,
        },
        SyntaxKind::String => quoted(text).map(ValueKind::String),
        SyntaxKind::RawString => Some(ValueKind::String(raw(text).to_string())),
        _ => None,
    }
}

fn quoted(text: &str) -> Option<String> {
    let content = &text[1..text.len() - 1];
    match text.as_bytes()[0] {
        b'"' => escape::unescape(content),
        _ => Some(content.to_string()),
    }
}

fn raw(text: &str) -> &str {
    let hashes = text[1..].bytes().take_while(|byte| *byte == b'#').count();
    &text[hashes + 2..text.len() - hashes - 1]
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
            !child.kind().is_separator()
                && !matches!(
                    child.kind(),
                    SyntaxKind::LBrace
                        | SyntaxKind::RBrace
                        | SyntaxKind::LBracket
                        | SyntaxKind::RBracket
                )
        })
        .collect()
}

fn rejected(range: TextRange) -> Vec<ConfigError> {
    error(ConfigErrorKind::Rejected, range)
}

#[cfg(test)]
mod tests {
    use crate::{
        Commas, Config, ConfigErrorKind, Format, FormatConfig, IndentStyle, LineEnding, Patterns,
        load, read, test_support,
        value::{Value, ValueKind},
    };

    fn errors(text: &str) -> Vec<(String, &str)> {
        test_support::errors(text, Format::Nuon)
    }

    fn rejected() -> String {
        ConfigErrorKind::Rejected.to_string()
    }

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

    fn string(text: &str) -> ValueKind {
        ValueKind::String(text.to_string())
    }

    #[test]
    fn record_of_any_quoting_and_spacing_gives_settings() {
        let text = concat!(
            "# nuten\n",
            "{\n",
            "    line_width: 80 # c\n",
            "    'indent_style': tab,\n",
            "    \"indent_width\": 2, `line_ending`: \"crlf\"\n",
            "    commas: none respect_gitignore: false editorconfig: false\n",
            "}\n",
        );
        let config = load(text, Format::Nuon).unwrap();
        assert_eq!(
            config.format_without_file(),
            FormatConfig {
                line_width: 80,
                indent_style: IndentStyle::Tab,
                indent_width: 2,
                line_ending: LineEnding::CrLf,
                commas: Commas::None,
            }
        );
        let expected = Config {
            respect_gitignore: false,
            editorconfig: false,
            changes: config.changes.clone(),
            ..Config::default()
        };
        assert_eq!(config, expected);
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
                .format_without_file()
                .line_width,
            80
        );
    }

    #[test]
    fn field_split_across_lines_is_read() {
        let text = "{\n  line_width:\n  # c\n  80,\n  \"indent_width\" : 2,\n}";
        let config = load(text, Format::Nuon).unwrap();
        assert_eq!(
            (
                config.format_without_file().line_width,
                config.format_without_file().indent_width
            ),
            (80, 2)
        );
    }

    #[test]
    fn crlf_line_endings_are_read() {
        let text = "{\r\n  line_width: 80 # c\r\n}\r\n";
        assert_eq!(
            load(text, Format::Nuon)
                .unwrap()
                .format_without_file()
                .line_width,
            80
        );
    }

    #[test]
    fn schema_key_is_read_when_quoted() {
        let text = r#"{"$schema": "./nuten.schema.json", line_width: 80}"#;
        assert_eq!(
            load(text, Format::Nuon)
                .unwrap()
                .format_without_file()
                .line_width,
            80
        );
    }

    #[test]
    fn bare_patterns_are_read() {
        let text = "{include: [*.nu, **/*.nuon, src/a.nu], exclude: [vendor/x, .cache]}";
        let config = load(text, Format::Nuon).unwrap();
        assert_eq!(
            config.include,
            Patterns::new(["*.nu", "**/*.nuon", "src/a.nu"]).unwrap()
        );
        assert_eq!(
            config.exclude,
            Patterns::new(["vendor/x", ".cache"]).unwrap()
        );
    }

    #[test]
    fn word_is_what_nu_reads_it_as() {
        for (text, kind) in [
            ("{a: 5}", ValueKind::Int(5)),
            ("{a: -5}", ValueKind::Int(-5)),
            ("{a: 1_000}", ValueKind::Int(1000)),
            ("{a: 0x64}", ValueKind::Int(100)),
            ("{a: 1.5}", ValueKind::Float(1.5)),
            ("{a: 1e2}", ValueKind::Float(100.0)),
            ("{a: true}", ValueKind::Bool(true)),
            ("{a: null}", ValueKind::Null),
            ("{a: tab}", string("tab")),
            ("{a: script.nu}", string("script.nu")),
            ("{a: ../x}", string("../x")),
            ("{a: é}", string("é")),
            ("{a: x\"y\"}", string("x\"y\"")),
            ("{a: 0.2.0}", string("0.2.0")),
        ] {
            assert_eq!(field_value(text), kind, "{text}");
        }
    }

    #[test]
    fn quoted_string_is_its_content_with_escapes_decoded() {
        for (text, content) in [
            (r#"{a: "x y"}"#, "x y"),
            (r"{a: 'x\ty'}", r"x\ty"),
            ("{a: `x y`}", "x y"),
            (r#"{a: ""}"#, ""),
            ("{a: \"x\r\ny\"}", "x\r\ny"),
            (r#"{a: "t\u{61}b"}"#, "tab"),
            (r#"{a: "a\tb\\"}"#, "a\tb\\"),
        ] {
            assert_eq!(field_value(text), string(content), "{text}");
        }
    }

    #[test]
    fn raw_string_is_its_content() {
        for (text, content) in [
            ("{a: r#'x'#}", "x"),
            ("{a: r##'x'##}", "x"),
            ("{a: r#'a\\n'b'#}", "a\\n'b"),
            ("{a: r#''#}", ""),
        ] {
            assert_eq!(field_value(text), string(content), "{text}");
        }
    }

    #[test]
    fn list_holds_its_items() {
        let ValueKind::List(items) = field_value("{a: [lf, 1\n # c\n 'x', a:b,]}") else {
            panic!("no list");
        };
        let kinds: Vec<_> = items.into_iter().map(|item| item.kind).collect();
        assert_eq!(
            kinds,
            [string("lf"), ValueKind::Int(1), string("x"), string("a:b")]
        );
    }

    #[test]
    fn list_separated_by_pipes_holds_its_items() {
        let ValueKind::List(items) = field_value("{a: [| b, | c e>| 1 o+e>|]}") else {
            panic!("no list");
        };
        let kinds: Vec<_> = items.into_iter().map(|item| item.kind).collect();
        assert_eq!(kinds, [string("b"), string("c"), ValueKind::Int(1)]);
    }

    #[test]
    fn table_separated_by_pipes_keys_its_rows_by_its_columns() {
        assert_eq!(
            rows("{a: [[b | c]; [1 e>| 2]]}"),
            [vec![
                ("b".to_string(), ValueKind::Int(1)),
                ("c".to_string(), ValueKind::Int(2)),
            ]]
        );
    }

    #[test]
    fn value_of_type_no_setting_takes_is_read_by_type() {
        for (value, kind) in [
            ("1kb", ValueKind::Filesize),
            ("1sec", ValueKind::Duration),
            ("2024-01-01", ValueKind::Date),
            ("1..2", ValueKind::Range),
            ("..2", ValueKind::Range),
            ("0x[ff]", ValueKind::Binary),
            ("0x[]", ValueKind::Binary),
            ("$.a", ValueKind::CellPath),
            ("$.a.0?", ValueKind::CellPath),
        ] {
            let text = format!("{{a: {value}}}");
            assert_eq!(field_value(&text), kind, "{text}");
        }
    }

    #[test]
    fn setting_given_type_it_does_not_take_is_reported_by_type() {
        for (value, found) in [
            ("1kb", "a filesize"),
            ("1sec", "a duration"),
            ("2024-01-01", "a date"),
            ("1..2", "a range"),
            ("0x[ff]", "a binary"),
            ("$.a", "a cell path"),
        ] {
            let text = format!("{{line_width: {value}}}");
            let message = format!("`line_width` takes an integer, not {found}");
            assert_eq!(errors(&text), [(message, value)], "{text}");
        }
    }

    fn rows(text: &str) -> Vec<Vec<(String, ValueKind)>> {
        let ValueKind::List(rows) = field_value(text) else {
            panic!("{text} holds no table");
        };
        rows.into_iter()
            .map(|row| {
                let ValueKind::Record(fields) = row.kind else {
                    panic!("{text} has a row that is no record");
                };
                fields
                    .into_iter()
                    .map(|field| (field.key, field.value.kind))
                    .collect()
            })
            .collect()
    }

    #[test]
    fn table_is_list_of_records_keyed_by_its_columns() {
        assert_eq!(
            rows("{a: [[b, \"c d\", e.f]; [1 x 2] [true, null, \"g\"]]}"),
            [
                vec![
                    ("b".to_string(), ValueKind::Int(1)),
                    ("c d".to_string(), string("x")),
                    ("e.f".to_string(), ValueKind::Int(2)),
                ],
                vec![
                    ("b".to_string(), ValueKind::Bool(true)),
                    ("c d".to_string(), ValueKind::Null),
                    ("e.f".to_string(), string("g")),
                ],
            ]
        );
        let nested = rows("{a: [[b, c]; [[2] {g: 1}]]}");
        assert!(matches!(&nested[0][0].1, ValueKind::List(items) if items.len() == 1));
        assert!(matches!(&nested[0][1].1, ValueKind::Record(fields) if fields[0].key == "g"));
        assert_eq!(rows("{a: [[]; []]}"), [Vec::new()]);
        assert_eq!(
            rows("{a: [[b]; [x:y]]}"),
            [vec![("b".to_string(), string("x:y"))]]
        );
        assert_eq!(
            rows("{a: [[b]; [1], [2],]}"),
            [
                vec![("b".to_string(), ValueKind::Int(1))],
                vec![("b".to_string(), ValueKind::Int(2))],
            ]
        );
    }

    #[test]
    fn table_row_with_other_number_of_cells_is_rejected() {
        for (text, at) in [
            ("{a: [[b c]; [1]]}", "[1]"),
            ("{a: [[b]; [1 2]]}", "[1 2]"),
            ("{a: [[]; [1]]}", "[1]"),
        ] {
            assert_eq!(errors(text), [(rejected(), at)], "{text}");
        }
    }

    #[test]
    fn table_column_that_is_no_string_is_rejected() {
        for (text, at) in [
            ("{a: [[1]; [1]]}", "1"),
            ("{a: [[true]; [1]]}", "true"),
            ("{a: [[2024-01-01]; [1]]}", "2024-01-01"),
            ("{a: [[r#'b'#]; [1]]}", "r#'b'#"),
            ("{a: [[[b]]; [1]]}", "[b]"),
        ] {
            assert_eq!(errors(text), [(rejected(), at)], "{text}");
        }
    }

    #[test]
    fn table_duplicate_column_is_reported_at_later_column() {
        assert_eq!(
            errors("{a: [[b, 'b']; [1 2]]}"),
            [("duplicate key `b`".to_string(), "'b'")]
        );
    }

    #[test]
    fn table_reports_every_duplicate_column_before_its_rows() {
        assert_eq!(
            errors("{a: [[b, \"b\", c, 'c']; [1]]}"),
            [
                ("duplicate key `b`".to_string(), "\"b\""),
                ("duplicate key `c`".to_string(), "'c'"),
            ]
        );
    }

    #[test]
    fn table_cell_nu_rejects_is_rejected() {
        assert_eq!(errors("{a: [[b]; [$x]]}"), [(rejected(), "$x")]);
    }

    #[test]
    fn value_nu_rejects_is_rejected() {
        for value in [
            "$x",
            "$x.a",
            "$x..2",
            "(1)..2",
            "1..$x",
            "$\"x\"",
            "(1)",
            "{|| 1}",
            "a(b)",
            "a:b",
            "0xg",
            "1-2b",
            "\"a\"b",
            r#""\q""#,
            r#""\xff""#,
        ] {
            let text = format!("{{a: {value}}}");
            assert_eq!(errors(&text), [(rejected(), value)], "{text}");
        }
    }

    #[test]
    fn key_of_any_word_but_keyword_is_string() {
        for (key, expected) in [
            ("1", "1"),
            ("1.5", "1.5"),
            ("inf", "inf"),
            ("2024-01-01", "2024-01-01"),
            ("1kb", "1kb"),
            ("*.nu", "*.nu"),
            ("-x", "-x"),
            (r#""a\tb""#, "a\tb"),
            ("`a b`", "a b"),
        ] {
            let text = format!("{{{key}: 1}}");
            let Ok(Value {
                kind: ValueKind::Record(fields),
                ..
            }) = read(&text, Format::Nuon)
            else {
                panic!("{text} is no record");
            };
            assert_eq!(fields[0].key, expected, "{text}");
        }
    }

    #[test]
    fn key_nu_rejects_is_rejected() {
        for key in [
            "true", "false", "null", "$schema", "a(b)", "r#'x'#", r#""\q""#,
        ] {
            let text = format!("{{{key}: 1}}");
            assert_eq!(errors(&text), [(rejected(), key)], "{text}");
        }
    }

    #[test]
    fn irregular_entry_is_rejected() {
        for (text, at) in [
            ("{a: 1 b}", &["b"][..]),
            ("{a: 1; b: 2}", &["; b:", "2"]),
            ("{...$x}", &["...$x"]),
            ("{a: 1} o> x", &["{a: 1} o> x"]),
        ] {
            let expected: Vec<_> = at.iter().map(|at| (rejected(), *at)).collect();
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
    fn byte_order_mark_is_rejected() {
        let text = "\u{feff}{}";
        assert_eq!(errors(text), [(rejected(), text)]);
    }
}
