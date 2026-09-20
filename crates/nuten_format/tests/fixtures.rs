//! Each `test_data/<name>.nu` must format to `test_data/<name>.expected.nu`.
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

use nuten_format::{Config, format, invariants};

use crate::support::caught;

#[test]
fn fixtures_format_as_expected() {
    let fixtures = fixtures();
    assert!(!fixtures.is_empty(), "no fixtures");
    let config = Config::default();
    let failures: Vec<_> = fixtures
        .iter()
        .flat_map(|fixture| fixture.failures(&config))
        .collect();
    assert!(
        failures.is_empty(),
        "failing fixtures:\n\n{}",
        failures.join("\n\n")
    );
}

fn fixtures() -> Vec<Fixture> {
    let dir = test_data();
    let stems: BTreeSet<_> = fs::read_dir(&dir)
        .unwrap_or_else(|error| panic!("{}: {error}", dir.display()))
        .filter_map(|entry| {
            let file_name = entry
                .unwrap_or_else(|error| panic!("an entry of {}: {error}", dir.display()))
                .file_name();
            let name = file_name
                .to_str()
                .unwrap_or_else(|| panic!("{}: not UTF-8", dir.join(&file_name).display()));
            stem_of(name)
        })
        .collect();
    stems.iter().map(|stem| Fixture::of(stem)).collect()
}

fn test_data() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("test_data")
}

/// The fixture `name` belongs to, whichever of its two files it names.
/// A file left without its pair then stands as a fixture that fails to read.
fn stem_of(name: &str) -> Option<String> {
    let stem = name.strip_suffix(".nu")?;
    Some(stem.strip_suffix(".expected").unwrap_or(stem).to_owned())
}

struct Fixture {
    src_name: String,
    expected_name: String,
}

impl Fixture {
    fn of(stem: &str) -> Fixture {
        Fixture {
            src_name: format!("{stem}.nu"),
            expected_name: format!("{stem}.expected.nu"),
        }
    }

    fn failures(&self, config: &Config) -> Vec<String> {
        match (
            contents_of(&self.src_name),
            contents_of(&self.expected_name),
        ) {
            (Ok(src), Ok(expected)) => [
                failures_in(&self.src_name, &src, &expected, config),
                failures_in(&self.expected_name, &expected, &expected, config),
            ]
            .into_iter()
            .flatten()
            .collect(),
            (read_src, read_expected) => [
                (&self.src_name, read_src),
                (&self.expected_name, read_expected),
            ]
            .into_iter()
            .filter_map(|(name, read)| read.err().map(|error| format!("{name}\n{error}")))
            .collect(),
        }
    }
}

fn contents_of(name: &str) -> io::Result<String> {
    fs::read_to_string(test_data().join(name))
}

/// What the checks find wrong with `name`.
/// Its contents are `src`, and it must format to `expected`.
fn failures_in(name: &str, src: &str, expected: &str, config: &Config) -> Vec<String> {
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
        .map(|detail| format!("{name}\n{detail}"))
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
