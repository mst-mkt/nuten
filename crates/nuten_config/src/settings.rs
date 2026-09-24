//! The settings a config file may hold, and the `Config` they make.
//!
//! Each setting says what it takes and what it changes in a `Config`, in one place.
//! The loader and the JSON Schema both read them from here.

use crate::{
    Config, ConfigError, ConfigErrorKind, IndentStyle, LineEnding, error,
    value::{self, Field, Value, ValueKind},
};

#[derive(Debug)]
pub(crate) struct Setting {
    pub(crate) key: &'static str,
    /// What an editor shows for the key.
    pub(crate) description: &'static str,
    pub(crate) kind: Kind,
}

#[derive(Debug)]
pub(crate) enum Kind {
    /// Both ends included.
    Integer {
        min: i64,
        max: i64,
        /// Takes an integer from `min` to `max`.
        set: fn(Config, i64) -> Config,
    },
    Choice(&'static [Choice]),
    /// Any string, which changes nothing.
    String,
}

#[derive(Debug)]
pub(crate) struct Choice {
    pub(crate) name: &'static str,
    pub(crate) set: fn(Config) -> Config,
}

pub(crate) const SETTINGS: [Setting; 5] = [
    Setting {
        key: "$schema",
        description: "Where an editor finds the JSON Schema; nuten does not read it.",
        kind: Kind::String,
    },
    Setting {
        key: "line_width",
        description: "Columns a line may take before it breaks where it can.",
        kind: Kind::Integer {
            min: 1,
            max: u32::MAX as i64,
            set: |config, width| Config {
                line_width: u32::try_from(width).expect("the width is from min to max"),
                ..config
            },
        },
    },
    Setting {
        key: "indent_style",
        description: "Whether to indent with spaces or with tabs.",
        kind: Kind::Choice(&[
            Choice {
                name: "space",
                set: |config| Config {
                    indent_style: IndentStyle::Space,
                    ..config
                },
            },
            Choice {
                name: "tab",
                set: |config| Config {
                    indent_style: IndentStyle::Tab,
                    ..config
                },
            },
        ]),
    },
    Setting {
        key: "indent_width",
        description: "Columns per indent level, and per tab when indenting with tabs.",
        kind: Kind::Integer {
            min: 1,
            max: u8::MAX as i64,
            set: |config, width| Config {
                indent_width: u8::try_from(width).expect("the width is from min to max"),
                ..config
            },
        },
    },
    Setting {
        key: "line_ending",
        description: "What ends each line.",
        kind: Kind::Choice(&[
            Choice {
                name: "lf",
                set: |config| Config {
                    line_ending: LineEnding::Lf,
                    ..config
                },
            },
            Choice {
                name: "crlf",
                set: |config| Config {
                    line_ending: LineEnding::CrLf,
                    ..config
                },
            },
        ]),
    },
];

#[derive(Debug, Clone, Copy)]
enum Change {
    Integer(fn(Config, i64) -> Config, i64),
    Choice(fn(Config) -> Config),
    Nothing,
}

impl Change {
    fn apply(self, config: Config) -> Config {
        match self {
            Change::Integer(set, integer) => set(config, integer),
            Change::Choice(set) => set(config),
            Change::Nothing => config,
        }
    }
}

pub(crate) fn config(value: &Value) -> Result<Config, Vec<ConfigError>> {
    let changes = value::all(record_fields(value)?.iter().map(change))?;
    Ok(changes
        .into_iter()
        .fold(Config::default(), |config, change| change.apply(config)))
}

fn record_fields(value: &Value) -> Result<&[Field], Vec<ConfigError>> {
    match &value.kind {
        ValueKind::Record(fields) => Ok(fields),
        kind => Err(error(
            ConfigErrorKind::NotRecord {
                found: kind.name(),
            },
            value.range,
        )),
    }
}

fn change(field: &Field) -> Result<Change, Vec<ConfigError>> {
    let Some(setting) = SETTINGS.iter().find(|setting| setting.key == field.key) else {
        return Err(error(
            ConfigErrorKind::UnknownKey(field.key.clone()),
            field.key_range,
        ));
    };
    let at_value = |kind| error(kind, field.value.range);
    let wrong_type = |expected| {
        at_value(ConfigErrorKind::WrongType {
            key: setting.key,
            expected,
            found: field.value.kind.name(),
        })
    };
    match (&setting.kind, &field.value.kind) {
        (
            &Kind::Integer {
                min,
                max,
                set,
            },
            &ValueKind::Int(integer),
        ) => {
            if (min..=max).contains(&integer) {
                Ok(Change::Integer(set, integer))
            } else {
                Err(at_value(ConfigErrorKind::OutOfRange {
                    key: setting.key,
                    min,
                    max,
                }))
            }
        },
        (
            Kind::Integer {
                ..
            },
            _,
        ) => Err(wrong_type("an integer")),
        (Kind::Choice(choices), ValueKind::String(string)) => choices
            .iter()
            .find(|choice| choice.name == string)
            .map(|choice| Change::Choice(choice.set))
            .ok_or_else(|| {
                at_value(ConfigErrorKind::UnknownChoice {
                    key: setting.key,
                    choices: choices.iter().map(|choice| choice.name).collect(),
                })
            }),
        (Kind::String, ValueKind::String(_)) => Ok(Change::Nothing),
        (Kind::Choice(_) | Kind::String, _) => Err(wrong_type("a string")),
    }
}

#[cfg(test)]
mod tests {
    use super::{Kind, SETTINGS};
    use crate::{Config, Format, IndentStyle, LineEnding, load, test_support::errors};

    #[test]
    fn given_settings_replace_defaults() {
        let text = r#"{
            "line_width": 80,
            "indent_style": "tab",
            "indent_width": 2,
            "line_ending": "crlf"
        }"#;
        let expected = Config {
            line_width: 80,
            indent_style: IndentStyle::Tab,
            indent_width: 2,
            line_ending: LineEnding::CrLf,
        };
        assert_eq!(load(text, Format::Json), Ok(expected));
    }

    #[test]
    fn settings_not_given_keep_defaults() {
        assert_eq!(load("{}", Format::Json), Ok(Config::default()));
        let config = load(r#"{"indent_width": 2}"#, Format::Json).unwrap();
        assert_eq!(
            config,
            Config {
                indent_width: 2,
                ..Config::default()
            }
        );
    }

    #[test]
    fn each_choice_gives_its_own_config() {
        for setting in &SETTINGS {
            let Kind::Choice(choices) = setting.kind else {
                continue;
            };
            let configs: Vec<_> = choices
                .iter()
                .map(|choice| {
                    let text = format!(r#"{{"{}": "{}"}}"#, setting.key, choice.name);
                    load(&text, Format::Json).unwrap()
                })
                .collect();
            for (index, config) in configs.iter().enumerate() {
                assert!(!configs[..index].contains(config), "{}", setting.key);
            }
        }
    }

    #[test]
    fn choices_set_what_they_name() {
        let config = |text| load(text, Format::Json).unwrap();
        assert_eq!(
            config(r#"{"indent_style": "space"}"#).indent_style,
            IndentStyle::Space
        );
        assert_eq!(
            config(r#"{"indent_style": "tab"}"#).indent_style,
            IndentStyle::Tab
        );
        assert_eq!(
            config(r#"{"line_ending": "lf"}"#).line_ending,
            LineEnding::Lf
        );
        assert_eq!(
            config(r#"{"line_ending": "crlf"}"#).line_ending,
            LineEnding::CrLf
        );
    }

    #[test]
    fn integers_at_either_end_of_range_are_taken() {
        let text = r#"{"line_width": 4294967295, "indent_width": 255}"#;
        let config = load(text, Format::Json).unwrap();
        assert_eq!(
            (config.line_width, config.indent_width),
            (u32::MAX, u8::MAX)
        );
        let text = r#"{"line_width": 1, "indent_width": 1}"#;
        let config = load(text, Format::Json).unwrap();
        assert_eq!((config.line_width, config.indent_width), (1, 1));
    }

    #[test]
    fn schema_key_takes_any_string() {
        let text = r#"{"$schema": "./nuten.schema.json"}"#;
        assert_eq!(load(text, Format::Json), Ok(Config::default()));
    }

    #[test]
    fn schema_key_changes_no_setting() {
        let text = r#"{"line_width": 80, "$schema": "x"}"#;
        assert_eq!(load(text, Format::Json).unwrap().line_width, 80);
    }

    #[test]
    fn schema_key_takes_nothing_but_string() {
        for (value, found) in [
            ("1", "an integer"),
            ("1.5", "a float"),
            ("null", "null"),
            ("true", "a boolean"),
            ("[]", "a list"),
            ("{}", "a record"),
        ] {
            let text = format!(r#"{{"$schema": {value}}}"#);
            assert_eq!(
                errors(&text, Format::Json),
                [(format!("`$schema` takes a string, not {found}"), value)]
            );
        }
    }

    #[test]
    fn unknown_key_is_reported_at_key() {
        assert_eq!(
            errors(r#"{"lineWidth": 80}"#, Format::Json),
            [("unknown key `lineWidth`".to_string(), r#""lineWidth""#)]
        );
    }

    #[test]
    fn value_of_wrong_type_is_reported_at_value() {
        for (text, message, at) in [
            (
                r#"{"line_width": "80"}"#,
                "`line_width` takes an integer, not a string",
                r#""80""#,
            ),
            (
                r#"{"line_width": 80.0}"#,
                "`line_width` takes an integer, not a float",
                "80.0",
            ),
            (
                r#"{"line_width": 1e2}"#,
                "`line_width` takes an integer, not a float",
                "1e2",
            ),
            (
                r#"{"indent_width": null}"#,
                "`indent_width` takes an integer, not null",
                "null",
            ),
            (
                r#"{"indent_style": true}"#,
                "`indent_style` takes a string, not a boolean",
                "true",
            ),
            (
                r#"{"line_ending": ["lf"]}"#,
                "`line_ending` takes a string, not a list",
                r#"["lf"]"#,
            ),
            (
                r#"{"line_ending": {}}"#,
                "`line_ending` takes a string, not a record",
                "{}",
            ),
        ] {
            assert_eq!(
                errors(text, Format::Json),
                [(message.to_string(), at)],
                "{text}"
            );
        }
    }

    #[test]
    fn integer_out_of_range_is_reported_at_value() {
        for (text, message, at) in [
            (
                r#"{"line_width": 0}"#,
                "`line_width` takes an integer from 1 to 4294967295",
                "0",
            ),
            (
                r#"{"line_width": 4294967296}"#,
                "`line_width` takes an integer from 1 to 4294967295",
                "4294967296",
            ),
            (
                r#"{"indent_width": -1}"#,
                "`indent_width` takes an integer from 1 to 255",
                "-1",
            ),
            (
                r#"{"indent_width": 256}"#,
                "`indent_width` takes an integer from 1 to 255",
                "256",
            ),
        ] {
            assert_eq!(
                errors(text, Format::Json),
                [(message.to_string(), at)],
                "{text}"
            );
        }
    }

    #[test]
    fn string_of_no_choice_is_reported_at_value() {
        for (text, message, at) in [
            (
                r#"{"indent_style": "Tab"}"#,
                "`indent_style` takes one of `space`, `tab`",
                r#""Tab""#,
            ),
            (
                r#"{"line_ending": "cr"}"#,
                "`line_ending` takes one of `lf`, `crlf`",
                r#""cr""#,
            ),
        ] {
            assert_eq!(
                errors(text, Format::Json),
                [(message.to_string(), at)],
                "{text}"
            );
        }
    }

    #[test]
    fn file_of_no_record_is_reported() {
        assert_eq!(
            errors("[]", Format::Json),
            [("config is a list, not a record".to_string(), "[]")]
        );
        assert_eq!(
            errors("1", Format::Json),
            [("config is an integer, not a record".to_string(), "1")]
        );
    }

    #[test]
    fn every_error_among_settings_is_reported_in_order() {
        let text = r#"{"indent_width": 0, "a": 1, "line_ending": "cr"}"#;
        assert_eq!(
            errors(text, Format::Json),
            [
                (
                    "`indent_width` takes an integer from 1 to 255".to_string(),
                    "0"
                ),
                ("unknown key `a`".to_string(), r#""a""#),
                (
                    "`line_ending` takes one of `lf`, `crlf`".to_string(),
                    r#""cr""#
                ),
            ]
        );
    }
}
