//! The config of each path, from the config file nearest it.

use std::{
    collections::{HashMap, HashSet},
    fs,
    path::{Component, Path, PathBuf},
};

use crate::{
    Config, Format,
    file::{located, read_regular_file},
    find::find_each,
};

/// The config of a path, or the problems that keep it from having one.
#[derive(Debug, Clone)]
pub enum Lookup {
    Found(Config),
    /// The path cannot be resolved, which puts the config of no other path in doubt.
    Unresolved(String),
    /// Its config file is wrong, unreadable, or cannot be told to exist.
    /// Formatting its files with another config would override the author's.
    Wrong(Vec<String>),
}

impl Lookup {
    pub fn is_wrong(&self) -> bool {
        matches!(self, Lookup::Wrong(_))
    }

    /// The error holds a line for each problem.
    pub fn into_config(self) -> Result<Config, Vec<String>> {
        match self {
            Lookup::Found(config) => Ok(config),
            lookup => Err(lookup.problems()),
        }
    }

    /// A line for each problem, none when it found a config.
    pub fn problems(&self) -> Vec<String> {
        match self {
            Lookup::Found(_) => Vec::new(),
            Lookup::Unresolved(problem) => vec![problem.clone()],
            Lookup::Wrong(problems) => problems.clone(),
        }
    }
}

/// The config of each of `files`: `given`, or else from the config file nearest the file.
/// Each file is canonical, as `canonical` gives it, or the problem resolving it.
pub fn of_files(files: &[Result<&Path, &str>], given: Option<&Config>) -> Vec<Lookup> {
    given_or_nearest(given, files.len(), || {
        files
            .iter()
            .map(|file| match file {
                Ok(file) => Ok(search_start(file)),
                Err(problem) => Err((*problem).to_string()),
            })
            .collect()
    })
}

/// The config of each of `dirs`, as `of_files` gives it for a file in the directory.
pub fn of_dirs(dirs: &[&Path], given: Option<&Config>) -> Vec<Lookup> {
    given_or_nearest(given, dirs.len(), || {
        dirs.iter().map(|dir| canonical(dir)).collect()
    })
}

/// The config of text not read from a file, such as stdin or a document in an editor.
/// It is that of `file`, where the text is to be written, or else of the current directory.
/// The error holds a line for each problem.
pub fn of_text(file: Option<&Path>) -> Result<Config, Vec<String>> {
    match file {
        // Found where its symlinks lead, as for any existing file.
        Some(file) if file.exists() => of_file(file),
        // An editor may not have written the file or its directories yet.
        Some(file) => of_dir(&nearest_existing_dir(file).map_err(|problem| vec![problem])?),
        None => of_dir(Path::new(".")),
    }
}

fn of_file(file: &Path) -> Result<Config, Vec<String>> {
    let file = canonical(file).map_err(|problem| vec![problem])?;
    of_files(&[Ok(&file)], None)
        .into_iter()
        .next()
        .expect("one lookup for one file")
        .into_config()
}

fn of_dir(dir: &Path) -> Result<Config, Vec<String>> {
    of_dirs(&[dir], None)
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
    let found = find_each(starts.iter().flatten().map(PathBuf::as_path));
    let config_files: HashSet<&Path> = starts
        .iter()
        .flatten()
        .filter_map(|dir| found[dir.as_path()].as_ref().ok()?.as_deref())
        .collect();
    let loaded: HashMap<&Path, Result<Config, Vec<String>>> = config_files
        .into_iter()
        .map(|file| (file, load_nearest(file)))
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

/// The directory the search for the config file of `file`, a canonical path, starts from.
fn search_start(file: &Path) -> PathBuf {
    file.parent().unwrap_or(file).to_path_buf()
}

/// `path` with symlinks resolved, since what it names is where they lead.
/// Resolves `..` too, since `find` reads it as written.
/// The error is the problem, as a line.
pub fn canonical(path: &Path) -> Result<PathBuf, String> {
    fs::canonicalize(path).map_err(|error| format!("{}: {error}", path.display()))
}

/// `file`, which may not exist yet, with the symlinks resolved on the way to it.
/// It is canonical when `file` exists, as `canonical` gives it.
/// The error is the problem, as a line.
pub fn resolved(file: &Path) -> Result<PathBuf, String> {
    if file.exists() {
        return canonical(file);
    }
    let name = file
        .file_name()
        .ok_or_else(|| format!("{}: names no file", file.display()))?;
    Ok(resolved_dir(file)?.join(name))
}

/// The error is the problem, as a line.
fn nearest_existing_dir(path: &Path) -> Result<PathBuf, String> {
    // None on a drive of Windows that does not exist.
    resolved_dir(path)?
        .ancestors()
        .find(|dir| dir.is_dir())
        .map(Path::to_path_buf)
        .ok_or_else(|| format!("{}: no directory above it exists", path.display()))
}

/// The directory `path` is in, resolved as far as it exists.
/// Each existing directory on the way is resolved, so a `..` after a symlink leaves its target.
/// A directory that does not exist yet holds no symlink, so a `..` after one resolves as written.
/// The error is the problem, as a line.
fn resolved_dir(path: &Path) -> Result<PathBuf, String> {
    // Not the file itself, which is no directory to go into even when it is a symlink to one.
    let parent = path.parent().unwrap_or(Path::new(""));
    // Not the current directory for an absolute path, since nuten may run in one that is removed.
    let start = if parent.is_absolute() {
        PathBuf::new()
    } else {
        canonical(Path::new("."))?
    };
    parent
        .components()
        .try_fold(start, |dir, component| match component {
            Component::ParentDir => Ok(dir.parent().map_or(dir.clone(), Path::to_path_buf)),
            Component::CurDir => Ok(dir),
            Component::Normal(name) => {
                let next = dir.join(name);
                if next.exists() {
                    canonical(&next)
                } else {
                    Ok(next)
                }
            },
            component => Ok(dir.join(component)),
        })
}

/// The config in `path`, the file `--config` names.
/// Its patterns are relative to the current directory.
/// The error holds a line for each problem.
pub fn load_given(path: &Path) -> Result<Config, Vec<String>> {
    let config = load_file(path)?;
    // Not resolved unless a pattern needs it, since nuten may run in a directory that is removed.
    if !config.needs_root() {
        return Ok(config);
    }
    let root = fs::canonicalize(".").map_err(|error| {
        vec![format!(
            "{}: its patterns are relative to the current directory, which cannot be resolved: \
             {error}",
            path.display()
        )]
    })?;
    Ok(Config {
        root: Some(root),
        ..config
    })
}

/// The config in `path`, a config file found for a path, with patterns relative to its directory.
/// `path` is in a canonical directory.
fn load_nearest(path: &Path) -> Result<Config, Vec<String>> {
    let config = load_file(path)?;
    Ok(Config {
        root: path.parent().map(Path::to_path_buf),
        ..config
    })
}

/// The error holds a line for each problem.
fn load_file(path: &Path) -> Result<Config, Vec<String>> {
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
    crate::load(&text, format).map_err(|errors| {
        errors
            .iter()
            .map(|error| located(path, &text, error.range.start().into(), &error.kind))
            .collect()
    })
}
