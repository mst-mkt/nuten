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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Config {
    pub format: FormatConfig,
    pub respect_gitignore: bool,
    pub include: Patterns,
    pub exclude: Patterns,
    pub overrides: Vec<Override>,
    /// It is canonical.
    pub root: Option<PathBuf>,
}

// Built once, since these stand for every file without a config file and a build is slow.
static DEFAULT_INCLUDE: LazyLock<Patterns> =
    LazyLock::new(|| Patterns::new(["*.nu", "*.nuon"]).expect("the patterns are valid"));
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
    pub fn depends_on_file(&self) -> bool {
        !self.exclude.is_empty() || !self.overrides.is_empty()
    }

    pub(crate) fn needs_root(&self) -> bool {
        self.include.has_anchored()
            || !self.exclude.is_empty()
            || self.overrides.iter().any(Override::needs_root)
    }

    /// `file` must be canonical, and `exclude` is the caller's to check.
    pub fn includes(&self, file: &Path) -> bool {
        self.include.matches_file(self.place(file))
    }

    /// `file` must be canonical.
    pub fn excludes(&self, file: &Path) -> bool {
        self.exclude.matches_file_or_ancestor(self.place(file))
    }

    /// `file` must be canonical.
    pub fn format_of(&self, file: &Path) -> FormatConfig {
        let place = self.place(file);
        self.overrides
            .iter()
            .filter(|picking| picking.picks(place))
            .fold(self.format.clone(), |format, picking| picking.apply(format))
    }

    fn place<'a>(&self, file: &'a Path) -> Place<'a> {
        Place::of(file, self.root.as_deref())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    Json,
    Jsonc,
    Nuon,
}

impl Format {
    pub const ALL: [Format; 3] = [Format::Nuon, Format::Jsonc, Format::Json];

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
    Json(String),
    Nuon(ParseErrorKind),
    Rejected,
    ExtraValue,
    DuplicateKey(String),
    NotRecord {
        found: &'static str,
    },
    UnknownKey(String),
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
    OverrideWithoutInclude,
    NotPerFile(&'static str),
}

impl fmt::Display for ConfigErrorKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ConfigErrorKind::Json(message) => f.write_str(message),
            ConfigErrorKind::Nuon(kind) => fmt::Display::fmt(kind, f),
            ConfigErrorKind::Rejected => f.write_str("value nu rejects"),
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

pub fn load(text: &str, format: Format) -> Result<Config, Vec<ConfigError>> {
    settings::config(&read(text, format)?)
}

pub fn read(text: &str, format: Format) -> Result<Value, Vec<ConfigError>> {
    match format {
        Format::Json => json::read(text, json::Dialect::Json),
        Format::Jsonc => json::read(text, json::Dialect::Jsonc),
        Format::Nuon => nuon::read(text),
    }
}

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
