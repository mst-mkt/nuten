//! Reads the settings of the formatter from a config file.
//!
//! A file is read into a `Value` first, whatever its format.
//! The settings are then taken from that.

mod json;
mod nuon;
mod settings;
#[cfg(test)]
mod test_support;
pub mod value;

use std::fmt;

pub use nuten_format::{Config, IndentStyle, LineEnding};
use nuten_parser::ParseErrorKind;
use nuten_syntax::TextRange;

use crate::value::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    /// JSON without comments or trailing commas.
    Json,
    /// JSON with comments and trailing commas, as VS Code reads it.
    Jsonc,
    /// Nushell Object Notation, as `from nuon` reads it.
    Nuon,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigError {
    pub kind: ConfigErrorKind,
    pub range: TextRange,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConfigErrorKind {
    /// Text the JSON parser rejects, with what it says.
    Json(String),
    /// NUON with a syntax error.
    Nuon(ParseErrorKind),
    /// A NUON value of a form nuten does not read, though nu may.
    Unreadable,
    /// A second value in a NUON file.
    ExtraValue,
    DuplicateKey(String),
    /// A file holding something other than a record; `found` names what.
    NotRecord {
        found: &'static str,
    },
    UnknownKey(String),
    /// A value of a type the setting does not take; `expected` and `found` name both.
    WrongType {
        key: &'static str,
        expected: &'static str,
        found: &'static str,
    },
    OutOfRange {
        key: &'static str,
        min: i64,
        max: i64,
    },
    UnknownChoice {
        key: &'static str,
        choices: Vec<&'static str>,
    },
}

impl fmt::Display for ConfigErrorKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ConfigErrorKind::Json(message) => f.write_str(message),
            ConfigErrorKind::Nuon(kind) => fmt::Display::fmt(kind, f),
            ConfigErrorKind::Unreadable => f.write_str(
                "value nuten does not read; it reads decimal integers, `true`, `false`, `null`, \
                 words of letters, digits, `_`, and `-` that start with a letter, \
                 strings without escapes, lists, and records",
            ),
            ConfigErrorKind::ExtraValue => f.write_str("value after the first; NUON holds one"),
            ConfigErrorKind::DuplicateKey(key) => write!(f, "duplicate key `{key}`"),
            ConfigErrorKind::NotRecord {
                found,
            } => {
                write!(f, "config is {found}, not a record")
            },
            ConfigErrorKind::UnknownKey(key) => write!(f, "unknown key `{key}`"),
            ConfigErrorKind::WrongType {
                key,
                expected,
                found,
            } => write!(f, "`{key}` takes {expected}, not {found}"),
            ConfigErrorKind::OutOfRange {
                key,
                min,
                max,
            } => {
                write!(f, "`{key}` takes an integer from {min} to {max}")
            },
            ConfigErrorKind::UnknownChoice {
                key,
                choices,
            } => {
                let choices: Vec<_> = choices.iter().map(|choice| format!("`{choice}`")).collect();
                write!(f, "`{key}` takes one of {}", choices.join(", "))
            },
        }
    }
}

/// Reads the settings `text` holds.
/// A setting it does not hold keeps its default.
/// The errors come in the order they appear in `text`, and there is at least one.
/// When `text` cannot be read, only the errors of reading it come, as `read` gives them.
pub fn load(text: &str, format: Format) -> Result<Config, Vec<ConfigError>> {
    settings::config(&read(text, format)?)
}

/// Reads `text` into a value, whatever settings it holds.
/// The errors come in the order they appear in `text`, and there is at least one.
/// Errors inside a list or a record hide the duplicate keys of the records around them.
pub fn read(text: &str, format: Format) -> Result<Value, Vec<ConfigError>> {
    match format {
        Format::Json => json::read(text, json::Dialect::Json),
        Format::Jsonc => json::read(text, json::Dialect::Jsonc),
        Format::Nuon => nuon::read(text),
    }
}

/// One error, as the readers and the settings give errors.
pub(crate) fn error(kind: ConfigErrorKind, range: TextRange) -> Vec<ConfigError> {
    vec![ConfigError {
        kind,
        range,
    }]
}
