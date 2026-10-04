//! Each setting says what it takes and which change it makes, in one place.
//! The loader and the JSON Schema both read them from here.

use nuten_syntax::TextRange;

use crate::{
    Commas, Config, ConfigError, ConfigErrorKind, FormatConfig, IndentStyle, LineEnding,
    NO_PATTERNS, Patterns, error,
    patterns::Place,
    value::{self, Field, Value, ValueKind},
};

#[derive(Debug)]
pub(crate) struct Setting {
    pub(crate) key: &'static str,
    pub(crate) description: &'static str,
    pub(crate) scope: Scope,
}

#[derive(Debug)]
pub(crate) enum Scope {
    File(Kind<FormatChange>),
    Whole(Kind<Change>),
}

#[derive(Debug)]
pub(crate) enum Kind<C: 'static> {
    Integer {
        min: i64,
        max: i64,
        change: fn(i64) -> C,
    },
    Choice(&'static [Choice<C>]),
    Boolean(fn(bool) -> C),
    Patterns(fn(Patterns) -> C),
    Overrides(fn(Vec<Override>) -> C),
    String,
}

#[derive(Debug)]
pub(crate) struct Choice<C> {
    pub(crate) name: &'static str,
    pub(crate) change: C,
}

pub(crate) static SETTINGS: [Setting; 10] = [
    Setting {
        key: "$schema",
        description: "Where an editor finds the JSON Schema; nuten does not read it.",
        scope: Scope::Whole(Kind::String),
    },
    Setting {
        key: "line_width",
        description: "Columns a line may take before it breaks where it can.",
        scope: Scope::File(Kind::Integer {
            min: 1,
            max: u32::MAX as i64,
            change: |width| {
                FormatChange::LineWidth(u32::try_from(width).expect("the width is from min to max"))
            },
        }),
    },
    Setting {
        key: "indent_style",
        description: "Whether to indent with spaces or with tabs.",
        scope: Scope::File(Kind::Choice(&[
            Choice {
                name: "space",
                change: FormatChange::IndentStyle(IndentStyle::Space),
            },
            Choice {
                name: "tab",
                change: FormatChange::IndentStyle(IndentStyle::Tab),
            },
        ])),
    },
    Setting {
        key: "indent_width",
        description: "Columns per indent level, and per tab when indenting with tabs.",
        scope: Scope::File(Kind::Integer {
            min: 1,
            max: u8::MAX as i64,
            change: |width| {
                FormatChange::IndentWidth(
                    u8::try_from(width).expect("the width is from min to max"),
                )
            },
        }),
    },
    Setting {
        key: "line_ending",
        description: "What ends each line, where `auto` follows the first line break outside a \
                      string, a bare word, or an `@example` block; line breaks inside those stay \
                      as written.",
        scope: Scope::File(Kind::Choice(&[
            Choice {
                name: "lf",
                change: FormatChange::LineEnding(LineEnding::Lf),
            },
            Choice {
                name: "crlf",
                change: FormatChange::LineEnding(LineEnding::CrLf),
            },
            Choice {
                name: "auto",
                change: FormatChange::LineEnding(LineEnding::Auto),
            },
        ])),
    },
    Setting {
        key: "commas",
        description: "Where a `,` goes among the items of lists, records, tables, parameters, and \
                      type parameters: nowhere, between items on one line, between items, or also \
                      after the last item when they break over lines.",
        scope: Scope::File(Kind::Choice(&[
            Choice {
                name: "none",
                change: FormatChange::Commas(Commas::None),
            },
            Choice {
                name: "inline",
                change: FormatChange::Commas(Commas::Inline),
            },
            Choice {
                name: "always",
                change: FormatChange::Commas(Commas::Always),
            },
            Choice {
                name: "trailing",
                change: FormatChange::Commas(Commas::Trailing),
            },
        ])),
    },
    Setting {
        key: "respect_gitignore",
        description: "Whether formatting a directory skips the files ignored by `.gitignore`, \
                      `.ignore`, the exclude file of the repository, or the global excludes file \
                      of git. The config file nearest the directory, or the one `--config` names, \
                      decides it for everything under the directory.",
        scope: Scope::Whole(Kind::Boolean(Change::RespectGitignore)),
    },
    Setting {
        key: "include",
        description: "Glob patterns of the files formatting a directory takes, in place of \
                      `*.nu` and `*.nuon`. A pattern without a `/` matches a file name at any \
                      depth; one with a `/` matches the path from the directory of the config \
                      file, or from the current directory for the config file `--config` names. \
                      A file named directly is formatted whether it matches or not.",
        scope: Scope::Whole(Kind::Patterns(Change::Include)),
    },
    Setting {
        key: "exclude",
        description: "Glob patterns of the files to leave alone, even when named directly, and \
                      of the directories whose files to leave alone. A pattern without a `/` \
                      matches a name at any depth; one with a `/` matches the path from the \
                      directory of the config file, or from the current directory for the \
                      config file `--config` names.",
        scope: Scope::Whole(Kind::Patterns(Change::Exclude)),
    },
    Setting {
        key: "overrides",
        description: "Settings for some files, in records of the settings that may differ from \
                      file to file. `include` picks the files of a record and `exclude` leaves \
                      some of them out, with patterns that match as the top-level ones do. A \
                      later record wins over an earlier one where both pick a file.",
        scope: Scope::Whole(Kind::Overrides(Change::Overrides)),
    },
];

pub(crate) const PICKERS: [(&str, &str); 2] = [
    (
        "include",
        "Glob patterns of the files the record is for; they match as those of the top-level \
         `include` do.",
    ),
    (
        "exclude",
        "Glob patterns of the files, and of the directories whose files, the record is not for \
         though `include` picks them; they match as those of the top-level `exclude` do.",
    ),
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FormatChange {
    LineWidth(u32),
    IndentStyle(IndentStyle),
    IndentWidth(u8),
    LineEnding(LineEnding),
    Commas(Commas),
}

impl FormatChange {
    fn apply(self, format: &FormatConfig) -> FormatConfig {
        match self {
            FormatChange::LineWidth(line_width) => FormatConfig {
                line_width,
                ..*format
            },
            FormatChange::IndentStyle(indent_style) => FormatConfig {
                indent_style,
                ..*format
            },
            FormatChange::IndentWidth(indent_width) => FormatConfig {
                indent_width,
                ..*format
            },
            FormatChange::LineEnding(line_ending) => FormatConfig {
                line_ending,
                ..*format
            },
            FormatChange::Commas(commas) => FormatConfig {
                commas,
                ..*format
            },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Change {
    Format(FormatChange),
    RespectGitignore(bool),
    Include(Patterns),
    Exclude(Patterns),
    Overrides(Vec<Override>),
}

impl Change {
    fn apply(self, config: Config) -> Config {
        match self {
            Change::Format(change) => Config {
                format: change.apply(&config.format),
                ..config
            },
            Change::RespectGitignore(respect_gitignore) => Config {
                respect_gitignore,
                ..config
            },
            Change::Include(include) => Config {
                include,
                ..config
            },
            Change::Exclude(exclude) => Config {
                exclude,
                ..config
            },
            Change::Overrides(overrides) => Config {
                overrides,
                ..config
            },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Override {
    include: Patterns,
    exclude: Patterns,
    changes: Vec<FormatChange>,
}

impl Override {
    pub(crate) fn picks(&self, place: Place<'_>) -> bool {
        self.include.matches_file(place) && !self.exclude.matches_file_or_ancestor(place)
    }

    pub(crate) fn needs_root(&self) -> bool {
        self.include.has_anchored() || !self.exclude.is_empty()
    }

    pub(crate) fn apply(&self, format: FormatConfig) -> FormatConfig {
        self.changes
            .iter()
            .fold(format, |format, change| change.apply(&format))
    }
}

pub(crate) fn config(value: &Value) -> Result<Config, Vec<ConfigError>> {
    let changes = value::all(record_fields(value)?.iter().map(change))?;
    Ok(changes
        .into_iter()
        .flatten()
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

fn change(field: &Field) -> Result<Option<Change>, Vec<ConfigError>> {
    change_of(setting(field)?, &field.value)
}

fn setting(field: &Field) -> Result<&'static Setting, Vec<ConfigError>> {
    SETTINGS
        .iter()
        .find(|setting| setting.key == field.key)
        .ok_or_else(|| {
            error(
                ConfigErrorKind::UnknownKey(field.key.clone()),
                field.key_range,
            )
        })
}

fn change_of(setting: &Setting, value: &Value) -> Result<Option<Change>, Vec<ConfigError>> {
    match &setting.scope {
        Scope::File(kind) => Ok(kind_change(setting.key, kind, value)?.map(Change::Format)),
        Scope::Whole(kind) => kind_change(setting.key, kind, value),
    }
}

fn kind_change<C: Clone>(
    key: &'static str,
    kind: &Kind<C>,
    value: &Value,
) -> Result<Option<C>, Vec<ConfigError>> {
    let at_value = |kind| error(kind, value.range);
    let wrong_type = |expected| {
        at_value(ConfigErrorKind::WrongType {
            key,
            expected,
            found: value.kind.name(),
        })
    };
    match (kind, &value.kind) {
        (
            &Kind::Integer {
                min,
                max,
                change,
            },
            &ValueKind::Int(integer),
        ) => {
            if (min..=max).contains(&integer) {
                Ok(Some(change(integer)))
            } else {
                Err(at_value(ConfigErrorKind::OutOfRange {
                    key,
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
            .map(|choice| Some(choice.change.clone()))
            .ok_or_else(|| {
                at_value(ConfigErrorKind::UnknownChoice {
                    key,
                    choices: choices.iter().map(|choice| choice.name).collect(),
                })
            }),
        (Kind::Boolean(change), &ValueKind::Bool(boolean)) => Ok(Some(change(boolean))),
        (Kind::Boolean(_), _) => Err(wrong_type("a boolean")),
        (Kind::Patterns(change), ValueKind::List(values)) => {
            Ok(Some(change(patterns(key, values)?)))
        },
        (Kind::Patterns(_), _) => Err(wrong_type("a list of strings")),
        (Kind::Overrides(change), ValueKind::List(values)) => {
            let overrides = values.iter().map(|value| match &value.kind {
                ValueKind::Record(fields) => override_of(fields, value.range),
                kind => Err(error(
                    ConfigErrorKind::WrongType {
                        key,
                        expected: "only records in its list",
                        found: kind.name(),
                    },
                    value.range,
                )),
            });
            Ok(Some(change(value::all(overrides)?)))
        },
        (Kind::Overrides(_), _) => Err(wrong_type("a list of records")),
        (Kind::String, ValueKind::String(_)) => Ok(None),
        (Kind::Choice(_) | Kind::String, _) => Err(wrong_type("a string")),
    }
}

fn override_of(fields: &[Field], range: TextRange) -> Result<Override, Vec<ConfigError>> {
    let changes = value::all(fields.iter().map(override_change))?;
    let include = changes.iter().find_map(|change| match change {
        Change::Include(patterns) => Some(patterns.clone()),
        _ => None,
    });
    let exclude = changes.iter().find_map(|change| match change {
        Change::Exclude(patterns) => Some(patterns.clone()),
        _ => None,
    });
    Ok(Override {
        include: include.ok_or_else(|| error(ConfigErrorKind::OverrideWithoutInclude, range))?,
        exclude: exclude.unwrap_or_else(|| NO_PATTERNS.clone()),
        changes: changes
            .into_iter()
            .filter_map(|change| match change {
                Change::Format(change) => Some(change),
                _ => None,
            })
            .collect(),
    })
}

fn override_change(field: &Field) -> Result<Change, Vec<ConfigError>> {
    let setting = setting(field)?;
    match change_of(setting, &field.value)? {
        Some(change @ (Change::Format(_) | Change::Include(_) | Change::Exclude(_))) => Ok(change),
        _ => Err(error(
            ConfigErrorKind::NotPerFile(setting.key),
            field.key_range,
        )),
    }
}

fn patterns(key: &'static str, values: &[Value]) -> Result<Patterns, Vec<ConfigError>> {
    let written = value::all(values.iter().map(|value| match &value.kind {
        ValueKind::String(pattern) => Ok(pattern.as_str()),
        kind => Err(error(
            ConfigErrorKind::WrongType {
                key,
                expected: "only strings in its list",
                found: kind.name(),
            },
            value.range,
        )),
    }))?;
    Patterns::new(written).map_err(|problems| {
        problems
            .into_iter()
            .map(|(index, problem)| ConfigError {
                kind: ConfigErrorKind::Pattern {
                    key,
                    problem,
                },
                range: values[index].range,
            })
            .collect()
    })
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use super::{Kind, SETTINGS, Scope};
    use crate::{
        Commas, Config, Format, FormatConfig, IndentStyle, LineEnding, Patterns, load,
        test_support::errors,
    };

    #[test]
    fn given_settings_replace_defaults() {
        let text = r#"{
            "line_width": 80,
            "indent_style": "tab",
            "indent_width": 2,
            "line_ending": "crlf",
            "commas": "trailing",
            "respect_gitignore": false,
            "include": ["*.nush"],
            "exclude": ["vendor"]
        }"#;
        let expected = Config {
            format: FormatConfig {
                line_width: 80,
                indent_style: IndentStyle::Tab,
                indent_width: 2,
                line_ending: LineEnding::CrLf,
                commas: Commas::Trailing,
            },
            respect_gitignore: false,
            include: Patterns::new(["*.nush"]).unwrap(),
            exclude: Patterns::new(["vendor"]).unwrap(),
            overrides: Vec::new(),
            root: None,
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
                format: FormatConfig {
                    indent_width: 2,
                    ..FormatConfig::default()
                },
                ..Config::default()
            }
        );
    }

    #[test]
    fn gitignore_is_respected_by_default() {
        assert!(Config::default().respect_gitignore);
    }

    #[test]
    fn respect_gitignore_takes_either_boolean() {
        for boolean in [true, false] {
            let text = format!(r#"{{"respect_gitignore": {boolean}}}"#);
            let config = load(&text, Format::Json).unwrap();
            assert_eq!(config.respect_gitignore, boolean, "{text}");
            assert_eq!(config.format, FormatConfig::default(), "{text}");
        }
    }

    #[test]
    fn each_choice_gives_its_own_config() {
        for setting in &SETTINGS {
            let names: Vec<_> = match setting.scope {
                Scope::File(Kind::Choice(choices)) => {
                    choices.iter().map(|choice| choice.name).collect()
                },
                Scope::Whole(Kind::Choice(choices)) => {
                    choices.iter().map(|choice| choice.name).collect()
                },
                _ => continue,
            };
            let configs: Vec<_> = names
                .iter()
                .map(|name| {
                    let text = format!(r#"{{"{}": "{name}"}}"#, setting.key);
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
            config(r#"{"indent_style": "space"}"#).format.indent_style,
            IndentStyle::Space
        );
        assert_eq!(
            config(r#"{"indent_style": "tab"}"#).format.indent_style,
            IndentStyle::Tab
        );
        assert_eq!(
            config(r#"{"line_ending": "lf"}"#).format.line_ending,
            LineEnding::Lf
        );
        assert_eq!(
            config(r#"{"line_ending": "crlf"}"#).format.line_ending,
            LineEnding::CrLf
        );
        assert_eq!(
            config(r#"{"line_ending": "auto"}"#).format.line_ending,
            LineEnding::Auto
        );
        for (name, commas) in [
            ("none", Commas::None),
            ("inline", Commas::Inline),
            ("always", Commas::Always),
            ("trailing", Commas::Trailing),
        ] {
            let text = format!(r#"{{"commas": "{name}"}}"#);
            let config = load(&text, Format::Json).unwrap();
            assert_eq!(config.format.commas, commas, "{text}");
        }
    }

    #[test]
    fn integers_at_either_end_of_range_are_taken() {
        let text = r#"{"line_width": 4294967295, "indent_width": 255}"#;
        let config = load(text, Format::Json).unwrap();
        assert_eq!(
            (config.format.line_width, config.format.indent_width),
            (u32::MAX, u8::MAX)
        );
        let text = r#"{"line_width": 1, "indent_width": 1}"#;
        let config = load(text, Format::Json).unwrap();
        assert_eq!(
            (config.format.line_width, config.format.indent_width),
            (1, 1)
        );
    }

    #[test]
    fn schema_key_takes_any_string() {
        let text = r#"{"$schema": "./nuten.schema.json"}"#;
        assert_eq!(load(text, Format::Json), Ok(Config::default()));
    }

    #[test]
    fn schema_key_changes_no_setting() {
        let text = r#"{"line_width": 80, "$schema": "x"}"#;
        assert_eq!(load(text, Format::Json).unwrap().format.line_width, 80);
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
            (
                r#"{"respect_gitignore": "false"}"#,
                "`respect_gitignore` takes a boolean, not a string",
                r#""false""#,
            ),
            (
                r#"{"respect_gitignore": 0}"#,
                "`respect_gitignore` takes a boolean, not an integer",
                "0",
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
                "`line_ending` takes one of `lf`, `crlf`, `auto`",
                r#""cr""#,
            ),
            (
                r#"{"commas": "never"}"#,
                "`commas` takes one of `none`, `inline`, `always`, `trailing`",
                r#""never""#,
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
    fn include_is_nu_and_nuon_files_and_exclude_is_nothing_by_default() {
        let config = Config::default();
        assert_eq!(config.include, Patterns::new(["*.nu", "*.nuon"]).unwrap());
        assert_eq!(config.exclude, Patterns::new([]).unwrap());
    }

    #[test]
    fn patterns_take_empty_list() {
        let config = load(r#"{"include": [], "exclude": []}"#, Format::Json).unwrap();
        assert_eq!(config.include, Patterns::new([]).unwrap());
        assert_eq!(config.exclude, Patterns::new([]).unwrap());
    }

    #[test]
    fn value_in_patterns_other_than_string_is_reported_at_value() {
        let text = r#"{"exclude": ["a", 1, null, ["b"]]}"#;
        assert_eq!(
            errors(text, Format::Json),
            [
                (
                    "`exclude` takes only strings in its list, not an integer".to_string(),
                    "1"
                ),
                (
                    "`exclude` takes only strings in its list, not null".to_string(),
                    "null"
                ),
                (
                    "`exclude` takes only strings in its list, not a list".to_string(),
                    r#"["b"]"#
                ),
            ]
        );
    }

    #[test]
    fn patterns_of_no_list_are_reported_at_value() {
        assert_eq!(
            errors(r#"{"include": "*.nu"}"#, Format::Json),
            [(
                "`include` takes a list of strings, not a string".to_string(),
                r#""*.nu""#
            )]
        );
    }

    #[test]
    fn each_pattern_with_problem_is_reported_at_pattern() {
        let text = r#"{"include": ["!a", "", "a/../b", "a[", "ok"]}"#;
        assert_eq!(
            errors(text, Format::Json),
            [
                (
                    "pattern of `include` starts with `!`, which negates nothing; `include` picks \
                     files, and `exclude` leaves them out"
                        .to_string(),
                    r#""!a""#
                ),
                ("pattern of `include` is empty".to_string(), r#""""#),
                (
                    "pattern of `include` has `..`, though patterns match only below the directory \
                     they are relative to"
                        .to_string(),
                    r#""a/../b""#
                ),
                (
                    "pattern of `include` is invalid: unclosed character class; missing ']'"
                        .to_string(),
                    r#""a[""#
                ),
            ]
        );
    }

    fn format_of(text: &str, file: &str) -> FormatConfig {
        let config = Config {
            root: Some(PathBuf::from("/r")),
            ..load(text, Format::Json).unwrap()
        };
        config.format_of(Path::new(file))
    }

    #[test]
    fn override_changes_format_of_files_it_picks() {
        let text = r#"{
            "indent_width": 2,
            "overrides": [{"include": ["d/*.nu"], "line_width": 80}]
        }"#;
        let picked = format_of(text, "/r/d/a.nu");
        assert_eq!((picked.line_width, picked.indent_width), (80, 2));
        let other = format_of(text, "/r/e/a.nu");
        assert_eq!((other.line_width, other.indent_width), (100, 2));
    }

    #[test]
    fn override_leaves_out_files_its_exclude_matches() {
        let text =
            r#"{"overrides": [{"include": ["*.nu"], "exclude": ["gen"], "indent_width": 2}]}"#;
        assert_eq!(format_of(text, "/r/a.nu").indent_width, 2);
        assert_eq!(format_of(text, "/r/gen/a.nu").indent_width, 4);
    }

    #[test]
    fn later_override_wins_where_both_pick_file() {
        let text = r#"{"overrides": [
            {"include": ["*.nu"], "indent_width": 2, "line_width": 80},
            {"include": ["a.nu"], "indent_width": 8}
        ]}"#;
        let a = format_of(text, "/r/a.nu");
        assert_eq!((a.indent_width, a.line_width), (8, 80));
        let b = format_of(text, "/r/b.nu");
        assert_eq!((b.indent_width, b.line_width), (2, 80));
    }

    #[test]
    fn overrides_change_nothing_but_format_of_file() {
        let text =
            r#"{"overrides": [{"include": ["d/*.nush"], "exclude": ["e"], "indent_width": 2}]}"#;
        let config = load(text, Format::Json).unwrap();
        assert_eq!(config.format, FormatConfig::default());
        assert_eq!(config.include, Config::default().include);
        assert_eq!(config.exclude, Config::default().exclude);
    }

    #[test]
    fn override_of_no_record_is_reported_at_value() {
        for (text, message, at) in [
            (
                r#"{"overrides": {}}"#,
                "`overrides` takes a list of records, not a record",
                "{}",
            ),
            (
                r#"{"overrides": [["a"]]}"#,
                "`overrides` takes only records in its list, not a list",
                r#"["a"]"#,
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
    fn override_without_include_is_reported_at_record() {
        let text = r#"{"overrides": [{"exclude": ["a"], "indent_width": 2}]}"#;
        assert_eq!(
            errors(text, Format::Json),
            [(
                "record of `overrides` lacks `include`, which picks its files".to_string(),
                r#"{"exclude": ["a"], "indent_width": 2}"#
            )]
        );
    }

    #[test]
    fn setting_same_for_every_file_is_reported_at_key_in_override() {
        let text =
            r#"{"overrides": [{"include": ["a"], "respect_gitignore": false, "overrides": []}]}"#;
        assert_eq!(
            errors(text, Format::Json),
            [
                (
                    "`respect_gitignore` cannot be set in `overrides`, since it is the same for \
                     every file"
                        .to_string(),
                    r#""respect_gitignore""#
                ),
                (
                    "`overrides` cannot be set in `overrides`, since it is the same for every file"
                        .to_string(),
                    r#""overrides""#
                ),
            ]
        );
    }

    #[test]
    fn every_error_in_overrides_is_reported_in_order() {
        let text = r#"{"overrides": [
            {"indent_width": 0, "include": ["!a"]},
            {"include": ["a"], "x": 1}
        ]}"#;
        let reported: Vec<_> = errors(text, Format::Json)
            .into_iter()
            .map(|(_, at)| at)
            .collect();
        assert_eq!(reported, ["0", r#""!a""#, r#""x""#]);
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
                    "`line_ending` takes one of `lf`, `crlf`, `auto`".to_string(),
                    r#""cr""#
                ),
            ]
        );
    }
}
