//! The config each file is formatted with, and each directory is walked with.

use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
};

use nuten_config::{Config, Format, find};

use crate::{located, read_regular_file, unique};

/// The config of a path, or the problems that keep it from having one.
#[derive(Debug, Clone)]
pub(crate) enum Lookup {
    Found(Config),
    /// The path cannot be resolved, which puts the config of no other path in doubt.
    Unresolved(String),
    /// Its config file is wrong, unreadable, or cannot be told to exist.
    /// Formatting its files with another config would override the author's.
    Wrong(Vec<String>),
}

impl Lookup {
    pub(crate) fn is_wrong(&self) -> bool {
        matches!(self, Lookup::Wrong(_))
    }

    pub(crate) fn into_config(self) -> Result<Config, Vec<String>> {
        match self {
            Lookup::Found(config) => Ok(config),
            lookup => Err(lookup.problems()),
        }
    }

    pub(crate) fn problems(&self) -> Vec<String> {
        match self {
            Lookup::Found(_) => Vec::new(),
            Lookup::Unresolved(problem) => vec![problem.clone()],
            Lookup::Wrong(problems) => problems.clone(),
        }
    }
}

/// The config of each of `files`: `given`, or else from the config file nearest the file.
pub(crate) fn of_files(files: &[&Path], given: Option<&Config>) -> Vec<Lookup> {
    given_or_nearest(given, files.len(), || {
        files.iter().map(|file| search_start(file)).collect()
    })
}

/// The config of each of `dirs`, as `of_files` gives it for a file in the directory.
pub(crate) fn of_dirs(dirs: &[&Path], given: Option<&Config>) -> Vec<Lookup> {
    given_or_nearest(given, dirs.len(), || {
        dirs.iter().map(|dir| canonical(dir)).collect()
    })
}

pub(crate) fn of_file(file: &Path, given: Option<&Config>) -> Result<Config, Vec<String>> {
    of_files(&[file], given)
        .into_iter()
        .next()
        .expect("one lookup for one file")
        .into_config()
}

pub(crate) fn of_dir(dir: &Path, given: Option<&Config>) -> Result<Config, Vec<String>> {
    of_dirs(&[dir], given)
        .into_iter()
        .next()
        .expect("one lookup for one directory")
        .into_config()
}

/// `starts` is lazy, since resolving it when `given` would report a path that cannot be resolved.
/// Formatting reports that path again.
fn given_or_nearest(
    given: Option<&Config>,
    count: usize,
    starts: impl FnOnce() -> Vec<Result<PathBuf, String>>,
) -> Vec<Lookup> {
    match given {
        Some(given) => vec![Lookup::Found(given.clone()); count],
        None => nearest(&starts()),
    }
}

/// The config in the config file nearest each of `starts`, the directories the search starts from.
fn nearest(starts: &[Result<PathBuf, String>]) -> Vec<Lookup> {
    let dirs = unique(starts.iter().flatten().map(PathBuf::as_path));
    let found: HashMap<&Path, Result<Option<PathBuf>, String>> = dirs
        .map(|dir| (dir, find(dir).map_err(|error| error.to_string())))
        .collect();
    let config_files = starts
        .iter()
        .flatten()
        .filter_map(|dir| found[dir.as_path()].as_ref().ok()?.as_deref());
    let loaded: HashMap<&Path, Result<Config, Vec<String>>> = unique(config_files)
        .map(|file| (file, load(file)))
        .collect();
    let lookup = |dir: &Path| match &found[dir] {
        Err(problem) => Lookup::Wrong(vec![problem.clone()]),
        Ok(None) => Lookup::Found(Config::default()),
        Ok(Some(file)) => match &loaded[file.as_path()] {
            Ok(config) => Lookup::Found(config.clone()),
            Err(problems) => Lookup::Wrong(problems.clone()),
        },
    };
    starts
        .iter()
        .map(|start| match start {
            Ok(dir) => lookup(dir),
            Err(problem) => Lookup::Unresolved(problem.clone()),
        })
        .collect()
}

/// The directory the search for the config file of `file` starts from.
fn search_start(file: &Path) -> Result<PathBuf, String> {
    let file = canonical(file)?;
    Ok(file
        .parent()
        .map_or_else(|| file.clone(), Path::to_path_buf))
}

/// `path` with symlinks resolved, since what it names is where they lead.
/// Resolves `..` too, since `find` reads it as written.
fn canonical(path: &Path) -> Result<PathBuf, String> {
    fs::canonicalize(path).map_err(|error| format!("{}: {error}", path.display()))
}

/// The error holds a line for each problem.
pub(crate) fn load(path: &Path) -> Result<Config, Vec<String>> {
    let Some(format) = Format::of_path(path) else {
        let extensions: Vec<_> = Format::ALL
            .iter()
            .map(|format| format!("`.{}`", format.extension()))
            .collect();
        return Err(vec![format!(
            "{}: config file needs one of the extensions {}",
            path.display(),
            extensions.join(", ")
        )]);
    };
    let text =
        read_regular_file(path).map_err(|error| vec![format!("{}: {error}", path.display())])?;
    nuten_config::load(&text, format).map_err(|errors| {
        errors
            .iter()
            .map(|error| located(path, &text, error.range.start().into(), &error.kind))
            .collect()
    })
}
