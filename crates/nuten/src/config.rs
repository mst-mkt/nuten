//! The config each file is formatted with, and each directory is walked with.

use std::{
    collections::HashMap,
    fmt::Display,
    fs,
    hash::Hash,
    path::{Path, PathBuf},
};

use nuten_config::{Config, Format, find};

use crate::{located, read_regular_file, unique};

/// A problem already reported to stderr.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Reported;

/// The config of each of `files`: `given`, or else from the config file nearest the file.
/// Reports every problem to stderr.
/// A file whose config file cannot be looked for gets an error.
/// Fails as a whole when any config file is wrong, unreadable, or cannot be told to exist.
/// Formatting with another config then would override the author's.
pub(crate) fn of_files(
    files: &[PathBuf],
    given: Option<&Config>,
) -> Result<Vec<Result<Config, Reported>>, Reported> {
    given_or_nearest(given, files.len(), || {
        files.iter().map(|file| search_start(file)).collect()
    })
}

/// The config of each of `dirs`, as `of_files` gives it for a file in the directory.
pub(crate) fn of_dirs(
    dirs: &[&Path],
    given: Option<&Config>,
) -> Result<Vec<Result<Config, Reported>>, Reported> {
    given_or_nearest(given, dirs.len(), || {
        dirs.iter().map(|dir| canonical(dir)).collect()
    })
}

/// `given` `count` times, or else the config nearest each of `starts`.
/// `starts` is lazy, since resolving it when `given` would report a path that cannot be resolved.
/// Formatting reports that path again.
fn given_or_nearest(
    given: Option<&Config>,
    count: usize,
    starts: impl FnOnce() -> Vec<Result<PathBuf, Reported>>,
) -> Result<Vec<Result<Config, Reported>>, Reported> {
    match given {
        Some(given) => Ok(vec![Ok(given.clone()); count]),
        None => nearest(&starts()),
    }
}

/// The config in the config file nearest each of `starts`, the directories the search starts from.
fn nearest(
    starts: &[Result<PathBuf, Reported>],
) -> Result<Vec<Result<Config, Reported>>, Reported> {
    // Both maps are collected in full before `settled`, so that every problem is reported.
    let found: HashMap<&Path, Result<Option<PathBuf>, Reported>> =
        unique(starts.iter().flatten().map(PathBuf::as_path))
            .map(|dir| (dir, find(dir).map_err(|error| report(&error))))
            .collect();
    // Not `found.values()`: the problems are reported in the order of `starts`.
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
fn search_start(file: &Path) -> Result<PathBuf, Reported> {
    let file = canonical(file)?;
    Ok(file
        .parent()
        .map_or_else(|| file.clone(), Path::to_path_buf))
}

/// `path` with symlinks resolved, since what it names is where they lead.
/// Resolves `..` too, since `find` reads it as written.
/// Reports the problem to stderr.
fn canonical(path: &Path) -> Result<PathBuf, Reported> {
    fs::canonicalize(path).map_err(|error| report(format!("{}: {error}", path.display())))
}

/// The settings in the config file `path`, in the format its extension names.
/// Reports every problem to stderr.
pub(crate) fn load(path: &Path) -> Result<Config, Reported> {
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
            let offset = error.range.start().into();
            report(located(path, &text, offset, &error.kind));
        }
        Reported
    })
}

fn report(message: impl Display) -> Reported {
    eprintln!("{message}");
    Reported
}
