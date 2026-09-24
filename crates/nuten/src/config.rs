//! The config each file is formatted with.

use std::{
    collections::{HashMap, HashSet},
    fmt::Display,
    fs,
    hash::Hash,
    path::{Path, PathBuf},
};

use nuten_config::{Config, Format, find};

use crate::{read_regular_file, report_at};

/// A problem already reported to stderr.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Reported;

/// The config of each of `files`: from `given`, or else from the config file nearest the file.
/// Reports every problem to stderr.
/// A file whose config file cannot be looked for gets an error.
/// Fails as a whole when any config file is wrong, unreadable, or cannot be told to exist.
/// Formatting with another config then would override the author's.
pub(crate) fn of_files(
    files: &[PathBuf],
    given: Option<&Path>,
) -> Result<Vec<Result<Config, Reported>>, Reported> {
    match given {
        Some(given) => load(given).map(|config| vec![Ok(config); files.len()]),
        None => nearest(files),
    }
}

fn nearest(files: &[PathBuf]) -> Result<Vec<Result<Config, Reported>>, Reported> {
    let starts: Vec<_> = files.iter().map(|file| search_start(file)).collect();
    // Both maps are collected in full before `settled`, so that every problem is reported.
    let found: HashMap<&Path, Result<Option<PathBuf>, Reported>> =
        unique(starts.iter().flatten().map(PathBuf::as_path))
            .map(|dir| (dir, find(dir).map_err(|error| report(&error))))
            .collect();
    // Not `found.values()`: the problems are reported in the order of the files.
    let config_files = starts
        .iter()
        .flatten()
        .filter_map(|dir| found[dir.as_path()].as_ref().ok()?.as_deref());
    let loaded: HashMap<&Path, Result<Config, Reported>> = unique(config_files)
        .map(|file| (file, load(file)))
        .collect();
    let found = settled(&found)?;
    let loaded = settled(&loaded)?;
    let config_of = |dir: &Path| {
        found[dir]
            .as_deref()
            .map_or_else(Config::default, |file| loaded[file].clone())
    };
    Ok(starts
        .iter()
        .map(|start| {
            start
                .as_deref()
                .map(config_of)
                .map_err(|reported| *reported)
        })
        .collect())
}

/// Each value of `results`, when none is an error.
fn settled<K: Copy + Eq + Hash, V>(
    results: &HashMap<K, Result<V, Reported>>,
) -> Result<HashMap<K, &V>, Reported> {
    results
        .iter()
        .map(|(key, result)| Ok((*key, result.as_ref().map_err(|reported| *reported)?)))
        .collect()
}

/// The directory the search for the config file of `file` starts from.
/// Reports the problem to stderr.
/// Resolves symlinks, since the file is where they lead.
/// Resolves `..` too, since `find` reads it as written.
fn search_start(file: &Path) -> Result<PathBuf, Reported> {
    let file =
        fs::canonicalize(file).map_err(|error| report(format!("{}: {error}", file.display())))?;
    Ok(file
        .parent()
        .map_or_else(|| file.clone(), Path::to_path_buf))
}

/// The settings in the config file `path`, in the format its extension names.
/// Reports every problem to stderr.
fn load(path: &Path) -> Result<Config, Reported> {
    let Some(format) = Format::of_path(path) else {
        let extensions: Vec<_> = Format::ALL
            .iter()
            .map(|format| format!("`.{}`", format.extension()))
            .collect();
        return Err(report(format!(
            "{}: config file needs one of the extensions {}",
            path.display(),
            extensions.join(", ")
        )));
    };
    let text =
        read_regular_file(path).map_err(|error| report(format!("{}: {error}", path.display())))?;
    nuten_config::load(&text, format).map_err(|errors| {
        for error in &errors {
            report_at(path, &text, error.range.start().into(), &error.kind);
        }
        Reported
    })
}

fn report(message: impl Display) -> Reported {
    eprintln!("{message}");
    Reported
}

/// Each path once, in the order it first comes.
fn unique<'a>(paths: impl Iterator<Item = &'a Path>) -> impl Iterator<Item = &'a Path> {
    let mut seen = HashSet::new();
    paths.filter(move |path| seen.insert(*path))
}
