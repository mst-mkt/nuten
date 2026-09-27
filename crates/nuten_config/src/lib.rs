//! Finds the config file of a path, reads the settings from it, and writes the JSON Schema of one.

mod escape;
pub mod file;
mod find;
mod json;
mod lookup;
mod nuon;
mod patterns;
mod schema;
mod settings;
#[cfg(test)]
mod test_support;
pub mod value;
mod word;

use std::{
    fmt,
    path::{Path, PathBuf},
    sync::LazyLock,
};

pub use nuten_format::{Commas, Config as FormatConfig, IndentStyle, LineEnding};
use nuten_parser::ParseErrorKind;
use nuten_syntax::TextRange;

pub use crate::{
    find::{FILE_NAMES, FindError, VCS_DIRS, find},
    lookup::{Lookup, canonical, load_given, of_dirs, of_files, of_text, resolved},
    patterns::{PatternProblem, Patterns},
    schema::schema,
    settings::Override,
};
use crate::{patterns::Place, value::Value};

/// What a config file sets.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Config {
    pub format: FormatConfig,
    /// Whether formatting a directory skips the files that ignore files ignore.
    pub respect_gitignore: bool,
    /// The files formatting a directory takes.
    pub include: Patterns,
    /// The files never formatted, and the directories whose files are never formatted.
    pub exclude: Patterns,
    /// Settings for the files some patterns pick, in the order they apply.
    pub overrides: Vec<Override>,
    /// The directory the patterns are relative to, canonical.
    /// None when no pattern needs it or the config is read from text alone.
    /// Only patterns without a `/` then match, and only the name of a file.
    pub root: Option<PathBuf>,
}

// Built once, since these stand for every file without a config file and a build is slow.
static DEFAULT_INCLUDE: LazyLock<Patterns> =
    LazyLock::new(|| Patterns::new(["*.nu", "*.nuon"]).expect("the patterns are valid"));
/// Also the `exclude` of an override without one.
pub(crate) static NO_PATTERNS: LazyLock<Patterns> =
    LazyLock::new(|| Patterns::new([]).expect("no pattern has a problem"));

impl Default for Config {
    fn default() -> Self {
        Self {
            format: FormatConfig::default(),
            respect_gitignore: true,
            include: DEFAULT_INCLUDE.clone(),
            exclude: NO_PATTERNS.clone(),
            overrides: Vec::new(),
            root: None,
        }
    }
}

impl Config {
    /// Whether how text is formatted depends on the file it is to be written to.
    pub fn depends_on_file(&self) -> bool {
        !self.exclude.is_empty() || !self.overrides.is_empty()
    }

    /// Whether the patterns need `root`, having a `/` or matching the directories above a file.
    pub(crate) fn needs_root(&self) -> bool {
        self.include.has_anchored()
            || !self.exclude.is_empty()
            || self.overrides.iter().any(Override::needs_root)
    }

    /// Whether formatting a directory takes `file`, a canonical path, unless `exclude` matches.
    pub fn includes(&self, file: &Path) -> bool {
        self.include.matches_file(self.place(file))
    }

    /// Whether `file`, a canonical path, is left alone even when named directly.
    pub fn excludes(&self, file: &Path) -> bool {
        self.exclude.matches_file_or_ancestor(self.place(file))
    }

    /// How to format `file`, a canonical path, with each override that picks it applied in order.
    pub fn format_of(&self, file: &Path) -> FormatConfig {
        let place = self.place(file);
        // Not from a clone of `self`, since an override changes only the format.
        let format = Config {
            format: self.format.clone(),
            ..Config::default()
        };
        self.overrides
            .iter()
            .filter(|picking| picking.picks(place))
            .fold(format, |config, picking| picking.apply(config))
            .format
    }

    fn place<'a>(&self, file: &'a Path) -> Place<'a> {
        Place::of(file, self.root.as_deref())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    /// JSON without comments or trailing commas.
    Json,
    /// JSON with comments and trailing commas, as VS Code reads it.
    Jsonc,
    /// Nushell Object Notation, as `from nuon` reads it.
    Nuon,
}

impl Format {
    /// In the order their files win in one directory.
    pub const ALL: [Format; 3] = [Format::Nuon, Format::Jsonc, Format::Json];

    /// What a file name in the format ends with, after the `.`.
    pub fn extension(self) -> &'static str {
        match self {
            Format::Json => "json",
            Format::Jsonc => "jsonc",
            Format::Nuon => "nuon",
        }
    }

    pub fn of_path(path: &Path) -> Option<Format> {
        let extension = path.extension()?;
        Format::ALL
            .into_iter()
            .find(|format| extension == format.extension())
    }
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
    Pattern {
        key: &'static str,
        problem: PatternProblem,
    },
    /// A record of `overrides` that picks no files, lacking `include`.
    OverrideWithoutInclude,
    /// A setting in `overrides` that is the same for every file.
    NotPerFile(&'static str),
}

impl fmt::Display for ConfigErrorKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ConfigErrorKind::Json(message) => f.write_str(message),
            ConfigErrorKind::Nuon(kind) => fmt::Display::fmt(kind, f),
            ConfigErrorKind::Unreadable => f.write_str(
                "value nuten does not read; it reads strings, integers, floats, `true`, `false`, \
                 `null`, lists, and records as nu does, and rejects what nu rejects",
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
            ConfigErrorKind::Pattern {
                key,
                problem,
            } => write!(f, "pattern of `{key}` {problem}"),
            ConfigErrorKind::OverrideWithoutInclude => {
                f.write_str("record of `overrides` lacks `include`, which picks its files")
            },
            ConfigErrorKind::NotPerFile(key) => write!(
                f,
                "`{key}` cannot be set in `overrides`, since it is the same for every file"
            ),
        }
    }
}

/// The errors come in the order they appear in `text`, and there is at least one.
/// When `text` cannot be read, only the errors of reading it come, as `read` gives them.
pub fn load(text: &str, format: Format) -> Result<Config, Vec<ConfigError>> {
    settings::config(&read(text, format)?)
}

/// Reads `text` into a value, whatever settings it holds.
/// The errors come in the order they appear in `text`, and there is at least one.
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

#[cfg(test)]
mod tests {
    use std::path::Path;

    use crate::Format;

    #[test]
    fn extension_names_format() {
        for (path, format) in [
            ("nuten.json", Some(Format::Json)),
            ("a/b.jsonc", Some(Format::Jsonc)),
            ("/a.b/c.nuon", Some(Format::Nuon)),
            ("nuten.toml", None),
            ("nuten.JSON", None),
            ("nuten", None),
            (".json", None),
            ("json", None),
            ("a.nuon/b", None),
        ] {
            assert_eq!(Format::of_path(Path::new(path)), format, "{path}");
        }
    }
}
