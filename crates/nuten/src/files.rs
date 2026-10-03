//! The files to format.

use std::{
    collections::{HashMap, HashSet},
    error::Error,
    ffi::OsStr,
    fs, io, iter,
    path::{Component, Path, PathBuf},
};

use ignore::{DirEntry, WalkBuilder, gitignore::Gitignore};
use nuten_config::{Lookup, VCS_DIRS};

/// A file that may be formatted, or a line for a problem met while looking for files.
pub(crate) type Entry = Result<Candidate, String>;

#[derive(Debug)]
pub(crate) struct Candidate {
    pub(crate) path: PathBuf,
    /// `path` with symlinks resolved, or the problem resolving it.
    pub(crate) canonical: Result<PathBuf, String>,
    /// Whether it is given directly, rather than found under a directory given.
    pub(crate) given: bool,
}

pub(crate) fn dirs(paths: &[PathBuf]) -> Vec<&Path> {
    paths
        .iter()
        .map(PathBuf::as_path)
        .filter(|path| path.is_dir())
        .collect()
}

/// A directory comes as its files and the problems met walking it, or as what keeps it unwalked.
/// `dir_lookups` holds the config of each of `dirs(paths)`, which says how to walk it.
pub(crate) fn expand(paths: &[PathBuf], dir_lookups: &HashMap<&Path, Lookup>) -> Vec<Entry> {
    let respects_gitignore =
        |lookup: &Lookup| matches!(lookup, Lookup::Found(config) if config.respect_gitignore);
    let global_problems = if dir_lookups.values().any(respects_gitignore) {
        global_ignore_problems()
    } else {
        Vec::new()
    };
    paths
        .iter()
        .flat_map(|path| match dir_lookups.get(path.as_path()) {
            None => vec![Ok(Candidate {
                path: path.clone(),
                canonical: nuten_config::canonical(path),
                given: true,
            })],
            // Not walked, since what the global excludes file of git ignores is unknown.
            Some(lookup) if respects_gitignore(lookup) && !global_problems.is_empty() => {
                global_problems.iter().cloned().map(Err).collect()
            },
            Some(Lookup::Found(config)) => walk(path, config.respect_gitignore),
            // Not walked, since the config to walk it with is unknown.
            Some(lookup) => lookup.problems().into_iter().map(Err).collect(),
        })
        .collect()
}

/// The problems with the global excludes file of git, whose lines the walker drops unseen.
fn global_ignore_problems() -> Vec<String> {
    Gitignore::global()
        .1
        .map_or_else(Vec::new, |error| problem_lines(&error))
}

/// Walks no symlink, since a link could lead out of `dir` or back into it.
/// Walks no directory of version control, since it holds no code of the author's.
/// Hidden files are walked, since config of Nushell lives in them.
fn walk(dir: &Path, respect_gitignore: bool) -> Vec<Entry> {
    // Directories covered by an ignore file with lines the walker drops.
    // Their files are left alone, since what those lines ignore is unknown.
    let mut untrusted: Vec<PathBuf> = Vec::new();
    // Resolved once, since the walk follows no symlink below it.
    let canonical_dir = nuten_config::canonical(dir);
    WalkBuilder::new(dir)
        .standard_filters(respect_gitignore)
        .hidden(false)
        .filter_entry(|entry| !is_vcs_dir(entry))
        .sort_by_file_name(OsStr::cmp)
        .build()
        .flat_map(|entry| {
            let entry = match entry {
                // An error apart from entries, other than IO, is from an ignore file above `dir`.
                // Its lines cover all of `dir`.
                Err(error) if !error.is_io() => {
                    untrusted.push(dir.to_path_buf());
                    return failed(&error);
                },
                Err(error) => return failed(&error),
                Ok(entry) => entry,
            };
            let path = entry.path();
            if untrusted
                .iter()
                .any(|untrusted| path.starts_with(untrusted))
            {
                return Vec::new();
            }
            if let Some(error) = entry.error() {
                untrusted.push(path.to_path_buf());
                return failed(error);
            }
            if is_regular_file(&entry) {
                let below = path
                    .strip_prefix(dir)
                    .expect("the walk yields paths under `dir`");
                vec![Ok(Candidate {
                    path: shown(path),
                    canonical: canonical_dir.clone().map(|dir| dir.join(below)),
                    given: false,
                })]
            } else {
                Vec::new()
            }
        })
        .collect()
}

fn failed(error: &ignore::Error) -> Vec<Entry> {
    problem_lines(error).into_iter().map(Err).collect()
}

fn is_vcs_dir(entry: &DirEntry) -> bool {
    entry
        .file_type()
        .is_some_and(|file_type| file_type.is_dir())
        && VCS_DIRS.iter().any(|name| entry.file_name() == *name)
}

fn is_regular_file(entry: &DirEntry) -> bool {
    entry
        .file_type()
        .is_some_and(|file_type| file_type.is_file())
}

/// `path` with no `./` before it, as a path in the current directory is written.
/// Neither a `.` inside it nor a `/` at its end is kept, so that one directory is named one way.
fn shown(path: &Path) -> PathBuf {
    let stripped: PathBuf = path
        .components()
        .skip_while(|component| *component == Component::CurDir)
        .collect();
    if stripped.as_os_str().is_empty() {
        PathBuf::from(".")
    } else {
        stripped
    }
}

/// Not the walker's own message, which names the path of an IO error twice.
fn problem_lines(error: &ignore::Error) -> Vec<String> {
    match error {
        ignore::Error::Partial(errors) => errors.iter().flat_map(problem_lines).collect(),
        ignore::Error::WithPath {
            path,
            err,
        } => {
            let line = match (&**err, err.io_error()) {
                (
                    ignore::Error::WithLineNumber {
                        line,
                        err,
                    },
                    _,
                ) => {
                    // Canonical, as the walker names the ignore files above the walk root.
                    // One ignore file is then named one way wherever it is met.
                    let path = fs::canonicalize(path).unwrap_or_else(|_| path.clone());
                    format!("{}:{line}: {err}", path.display())
                },
                (_, Some(io_error)) => {
                    format!("{}: {}", shown(path).display(), innermost(io_error))
                },
                _ => format!("{}: {err}", shown(path).display()),
            };
            vec![line]
        },
        _ => vec![error.to_string()],
    }
}

/// The IO error that caused `error` in the end.
fn innermost(error: &io::Error) -> &io::Error {
    let outermost: &(dyn Error + 'static) = error;
    iter::successors(Some(outermost), |&error| error.source())
        .filter_map(|error| error.downcast_ref::<io::Error>())
        .last()
        .unwrap_or(error)
}

/// Each of `entries` but those whose file, at `path_of` it, is the same file as one before them.
/// Nor those that are one of `excluded` under another name, since `exclude` leaves that file alone.
pub(crate) fn distinct<T>(
    entries: Vec<Result<T, String>>,
    excluded: &[PathBuf],
    path_of: impl Fn(&T) -> &PathBuf,
) -> Vec<Result<T, String>> {
    let mut seen: HashSet<_> = excluded.iter().map(|path| identity(path)).collect();
    entries
        .into_iter()
        .filter(|entry| {
            entry
                .as_ref()
                .map_or(true, |file| seen.insert(identity(path_of(file))))
        })
        .collect()
}

#[derive(Debug, PartialEq, Eq, Hash)]
enum Identity {
    File(FileId),
    /// A file whose identity cannot be read is known by its path.
    /// Reading it for formatting reports why.
    Path(PathBuf),
}

fn identity(path: &Path) -> Identity {
    file_id(path).map_or_else(|_| Identity::Path(path.to_path_buf()), Identity::File)
}

#[cfg(unix)]
type FileId = (u64, u64);

/// Not the path with symlinks resolved, which tells two hard links to one file apart.
#[cfg(unix)]
fn file_id(path: &Path) -> io::Result<FileId> {
    use std::os::unix::fs::MetadataExt;

    let metadata = fs::metadata(path)?;
    Ok((metadata.dev(), metadata.ino()))
}

#[cfg(not(unix))]
type FileId = PathBuf;

/// Stable std has no file identity outside unix, so two hard links to one file count as two here.
#[cfg(not(unix))]
fn file_id(path: &Path) -> io::Result<FileId> {
    fs::canonicalize(path)
}
