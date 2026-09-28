//! Each `test_data/**/<name>.input.nu` must format to `<name>.expected.nu` next to it.
//! The expected output must format to itself.
//! Formatting either must keep every invariant `nuten_format::invariants` asks about.
//! That covers those of the parser and of the formatter alike.
//!
//! A failing fixture does not stop the others.
//! A change to a rule then shows all it affects at once.

mod support;

use std::{
    collections::BTreeSet,
    fs, io,
    panic::UnwindSafe,
    path::{Path, PathBuf},
};

use nuten_format::{Commas, Config, LineEnding, format, invariants};

use crate::support::{caught, files_below};

const INPUT_SUFFIX: &str = ".input.nu";
const EXPECTED_SUFFIX: &str = ".expected.nu";

/// Below it, each directory of `CONFIGS` holds fixtures that format with its config.
/// Elsewhere, fixtures format with the default.
const CONFIGS_DIR: &str = "config";

/// Makes the config of a directory from the default.
type Change = fn(Config) -> Config;

/// Each directory, below `CONFIGS_DIR`, with how its config differs from the default.
const CONFIGS: &[(&str, Change)] = &[
    ("commas/none", |config| Config {
        commas: Commas::None,
        ..config
    }),
    ("commas/always", |config| Config {
        commas: Commas::Always,
        ..config
    }),
    ("commas/trailing", |config| Config {
        commas: Commas::Trailing,
        ..config
    }),
    ("line_ending/lf", |config| Config {
        line_ending: LineEnding::Lf,
        ..config
    }),
];

#[test]
fn test_data_holds_only_fixture_files() {
    let strays: Vec<_> = files_below_test_data()
        .iter()
        .filter(|name| stem_of(name).is_none())
        .map(|name| name.display().to_string())
        .collect();
    assert!(
        strays.is_empty(),
        "neither an input nor an expected output:\n{}",
        strays.join("\n")
    );
}

#[test]
fn fixture_files_format_to_expected_keeping_invariants() {
    let names = files_below_test_data();
    let stems: BTreeSet<_> = names.iter().filter_map(|name| stem_of(name)).collect();
    assert!(!stems.is_empty(), "no fixtures");
    let failures: Vec<_> = stems
        .iter()
        .flat_map(|stem| match config_of(stem) {
            Ok(config) => Fixture::of(stem).failures(&config),
            Err(problem) => vec![problem],
        })
        .collect();
    assert!(
        failures.is_empty(),
        "failing fixtures:\n\n{}",
        failures.join("\n\n")
    );
}

fn files_below_test_data() -> Vec<PathBuf> {
    let dir = test_data();
    files_below(&dir, &|_| true)
        .iter()
        .map(|path| path.strip_prefix(&dir).unwrap().to_path_buf())
        .collect()
}

fn test_data() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("test_data")
}

/// The config of the fixture `stem`: that of its directory below `CONFIGS_DIR`, or the default.
fn config_of(stem: &str) -> Result<Config, String> {
    let configs_dir = Path::new(CONFIGS_DIR);
    let stem = Path::new(stem);
    if !stem.starts_with(configs_dir) {
        return Ok(Config::default());
    }
    CONFIGS
        .iter()
        .find(|(dir, _)| stem.starts_with(configs_dir.join(dir)))
        .map(|(_, change)| change(Config::default()))
        .ok_or_else(|| format!("{}\nnot below a directory of CONFIGS", stem.display()))
}

/// The fixture `name` belongs to, whichever of its two files it names.
/// A file left without its pair then stands as a fixture that fails to read.
fn stem_of(name: &Path) -> Option<&str> {
    let name = name.to_str()?;
    name.strip_suffix(INPUT_SUFFIX)
        .or_else(|| name.strip_suffix(EXPECTED_SUFFIX))
}

struct Fixture {
    input_name: PathBuf,
    expected_name: PathBuf,
}

impl Fixture {
    fn of(stem: &str) -> Fixture {
        Fixture {
            input_name: format!("{stem}{INPUT_SUFFIX}").into(),
            expected_name: format!("{stem}{EXPECTED_SUFFIX}").into(),
        }
    }

    fn failures(&self, config: &Config) -> Vec<String> {
        match (
            contents_of(&self.input_name),
            contents_of(&self.expected_name),
        ) {
            (Ok(input), Ok(expected)) => [
                failures_in(&self.input_name, &input, &expected, config),
                failures_in(&self.expected_name, &expected, &expected, config),
            ]
            .into_iter()
            .flatten()
            .collect(),
            (read_input, read_expected) => [
                (&self.input_name, read_input),
                (&self.expected_name, read_expected),
            ]
            .into_iter()
            .filter_map(|(name, read)| {
                read.err()
                    .map(|error| format!("{}\n{error}", name.display()))
            })
            .collect(),
        }
    }
}

fn contents_of(name: &Path) -> io::Result<String> {
    fs::read_to_string(test_data().join(name))
}

/// What the checks find wrong with `name`.
/// Its contents are `src`, and it must format to `expected`.
fn failures_in(name: &Path, src: &str, expected: &str, config: &Config) -> Vec<String> {
    let formatting = detail_of(|| formats_to(src, expected, config));
    let invariant = detail_of(|| keeps_invariants(src, config));
    // Where both checks format, a panic in the rules reaches both, which report it alike.
    // It is one problem, not two.
    let details = match (formatting, invariant) {
        (Some(one), Some(other)) if one == other => vec![one],
        (formatting, invariant) => [formatting, invariant].into_iter().flatten().collect(),
    };
    details
        .into_iter()
        .map(|detail| format!("{}\n{detail}", name.display()))
        .collect()
}

/// What `check` finds wrong, a panic of nuten's own included.
fn detail_of(check: impl FnOnce() -> Result<(), String> + UnwindSafe) -> Option<String> {
    match caught(check) {
        Ok(outcome) => outcome.err(),
        Err(message) => Some(format!("panics: {message}")),
    }
}

fn formats_to(src: &str, expected: &str, config: &Config) -> Result<(), String> {
    let actual =
        format(src, config).map_err(|error| format!("does not parse: {:?}", error.errors))?;
    if actual == expected {
        Ok(())
    } else {
        Err(format!(
            "--- expected\n{expected}--- actual\n{actual}--- as strings\n{expected:?}\n{actual:?}"
        ))
    }
}

fn keeps_invariants(src: &str, config: &Config) -> Result<(), String> {
    // The syntax errors `check` also finds are left to `formats_to`, which fails on exactly them.
    let violations: Vec<_> = invariants::check(src, config)
        .violations
        .iter()
        .map(ToString::to_string)
        .collect();
    if violations.is_empty() {
        Ok(())
    } else {
        Err(violations.join("\n"))
    }
}
