//! The files to format.

use std::{
    collections::{HashMap, HashSet},
    error::Error,
    ffi::OsStr,
    fs, io, iter,
    path::{Component, Path, PathBuf},
};

use ignore::{DirEntry, WalkBuilder, gitignore::Gitignore};

use crate::{config::Reported, unique};

/// Directories of version control, which hold no code of the author's.
const VCS_DIRS: [&str; 5] = [".git", ".hg", ".svn", ".bzr", ".jj"];

#[derive(Debug)]
pub(crate) struct Expanded {
    pub(crate) files: Vec<PathBuf>,
    /// A line for each problem, each once.
    pub(crate) problems: Vec<String>,
}

/// The directories among `paths`.
pub(crate) fn dirs(paths: &[PathBuf]) -> Vec<&Path> {
    paths
        .iter()
        .map(PathBuf::as_path)
        .filter(|path| path.is_dir())
        .collect()
}

/// The files `paths` name, in their order: a file as it is, and a directory as its `.nu` files.
/// `respect_gitignore` tells for each of `dirs(paths)` whether to walk it respecting ignore files.
pub(crate) fn expand(
    paths: &[PathBuf],
    respect_gitignore: &HashMap<&Path, Result<bool, Reported>>,
) -> Expanded {
    let global_problems = if respect_gitignore
        .values()
        .any(|respect| matches!(respect, Ok(true)))
    {
        global_ignore_problems()
    } else {
        Vec::new()
    };
    let found: Vec<_> = paths
        .iter()
        .flat_map(|path| match respect_gitignore.get(path.as_path()) {
            None => vec![Ok(path.clone())],
            // Not walked, since the config to walk it with is unknown.
            // The problem has been reported already.
            Some(Err(_)) => Vec::new(),
            // Not walked, since what the global excludes file of git ignores is unknown.
            Some(Ok(true)) if !global_problems.is_empty() => Vec::new(),
            Some(&Ok(respect_gitignore)) => walk(path, respect_gitignore),
        })
        .collect();
    let walk_problems = found.iter().filter_map(|found| found.as_ref().err());
    let problems = unique(global_problems.iter().chain(walk_problems))
        .cloned()
        .collect();
    Expanded {
        files: found.into_iter().flatten().collect(),
        problems,
    }
}

/// The problems with the global excludes file of git, whose lines the walker drops unseen.
fn global_ignore_problems() -> Vec<String> {
    Gitignore::global()
        .1
        .map_or_else(Vec::new, |error| problem_lines(&error))
}

/// The `.nu` files under `dir` in path order, and the problems.
/// Leaves out the files that ignore files ignore when `respect_gitignore`.
/// Walks no symlink, since a link could lead out of `dir` or back into it.
/// Hidden files are walked, since config of Nushell lives in them.
fn walk(dir: &Path, respect_gitignore: bool) -> Vec<Result<PathBuf, String>> {
    // Directories covered by an ignore file with lines the walker drops.
    // Their files are left alone, since what those lines ignore is unknown.
    let mut untrusted: Vec<PathBuf> = Vec::new();
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
            if is_nu_file(&entry) {
                vec![Ok(shown(path))]
            } else {
                Vec::new()
            }
        })
        .collect()
}

/// The walk results that `error` makes, one for each problem.
fn failed(error: &ignore::Error) -> Vec<Result<PathBuf, String>> {
    problem_lines(error).into_iter().map(Err).collect()
}

fn is_vcs_dir(entry: &DirEntry) -> bool {
    entry
        .file_type()
        .is_some_and(|file_type| file_type.is_dir())
        && VCS_DIRS.iter().any(|name| entry.file_name() == *name)
}

fn is_nu_file(entry: &DirEntry) -> bool {
    entry
        .file_type()
        .is_some_and(|file_type| file_type.is_file())
        && entry.path().extension() == Some(OsStr::new("nu"))
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

/// A line for each problem in `error`.
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

/// Each of `files` but those that are the same file as one before them.
pub(crate) fn distinct(files: &[PathBuf]) -> Vec<PathBuf> {
    let mut seen = HashSet::new();
    files
        .iter()
        .filter(|file| seen.insert(identity(file)))
        .cloned()
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
