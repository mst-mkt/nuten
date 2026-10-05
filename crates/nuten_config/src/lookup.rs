use std::{
    collections::{HashMap, HashSet},
    error, fmt, fs, io,
    path::{Component, Path, PathBuf},
};

use nuten_line_index::{LineColumn, located};

use crate::{Config, Format, editorconfig, find::find_each};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct LookupError {
    pub path: PathBuf,
    pub line_column: Option<LineColumn>,
    pub message: String,
}

impl LookupError {
    pub(crate) fn new(path: &Path, message: impl fmt::Display) -> LookupError {
        LookupError {
            path: path.to_path_buf(),
            line_column: None,
            message: message.to_string(),
        }
    }
}

impl error::Error for LookupError {}

impl fmt::Display for LookupError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.path.display())?;
        if let Some(line_column) = self.line_column {
            write!(f, ":{}:{}", line_column.line, line_column.column)?;
        }
        write!(f, ": {}", self.message)
    }
}

#[derive(Debug, Clone)]
pub enum Lookup {
    Found(Config),
    /// The path cannot be resolved, which puts the config of no other path in doubt.
    Unresolved(LookupError),
    /// Formatting the files with another config would override the author's.
    Wrong(Vec<LookupError>),
}

impl Lookup {
    pub fn is_wrong(&self) -> bool {
        matches!(self, Lookup::Wrong(_))
    }

    pub fn into_config(self) -> Result<Config, Vec<LookupError>> {
        match self {
            Lookup::Found(config) => Ok(config),
            lookup => Err(lookup.errors()),
        }
    }

    pub fn errors(&self) -> Vec<LookupError> {
        match self {
            Lookup::Found(_) => Vec::new(),
            Lookup::Unresolved(error) => vec![error.clone()],
            Lookup::Wrong(errors) => errors.clone(),
        }
    }
}

/// Each file is canonical, as `canonical` gives it, or the error resolving it.
pub fn of_files(files: &[Result<&Path, &LookupError>], given: Option<&Config>) -> Vec<Lookup> {
    let starts: Vec<_> = files
        .iter()
        .map(|file| match file {
            Ok(file) => Ok(search_start(file)),
            Err(error) => Err((*error).clone()),
        })
        .collect();
    let lookups = given_or_nearest(given, files.len(), || nearest(&starts));
    with_editorconfig_files(
        lookups
            .into_iter()
            .zip(starts.iter().map(|start| start.as_deref().ok())),
    )
}

pub fn of_dirs(dirs: &[&Path], given: Option<&Config>) -> Vec<Lookup> {
    given_or_nearest(given, dirs.len(), || {
        let starts: Vec<_> = dirs.iter().map(|dir| canonical(dir)).collect();
        nearest(&starts)
    })
}

pub fn of_text(file: Option<&Path>, given: Option<&Config>) -> Result<Config, Vec<LookupError>> {
    match file {
        Some(file) if file.exists() => {
            let file = canonical(file).map_err(|error| vec![error])?;
            sole_config(of_files(&[Ok(&file)], given))
        },
        // An editor may not have written the file or its directories yet.
        Some(file) => of_unwritten(file, given),
        // Without a file, no section of `.editorconfig` could name it.
        None => sole_config(of_dirs(&[Path::new(".")], given)),
    }
}

fn of_unwritten(file: &Path, given: Option<&Config>) -> Result<Config, Vec<LookupError>> {
    match (nearest_existing_dir(file), given) {
        (Ok(dir), _) => {
            let lookups = of_dirs(&[&dir], given);
            sole_config(with_editorconfig_files(
                lookups.into_iter().zip([Some(dir.as_path())]),
            ))
        },
        // A file that is nowhere has no `.editorconfig` either.
        (Err(_), Some(given)) => Ok(given.clone()),
        (Err(error), None) => Err(vec![error]),
    }
}

fn sole_config(lookups: Vec<Lookup>) -> Result<Config, Vec<LookupError>> {
    lookups
        .into_iter()
        .next()
        .expect("one lookup for one path")
        .into_config()
}

/// `nearest` is lazy, since with a config given, resolving would report an unresolvable path.
/// Formatting reports that path again.
fn given_or_nearest(
    given: Option<&Config>,
    count: usize,
    nearest: impl FnOnce() -> Vec<Lookup>,
) -> Vec<Lookup> {
    match given {
        Some(given) => vec![Lookup::Found(given.clone()); count],
        None => nearest(),
    }
}

/// A lookup paired with no directory takes no `.editorconfig`.
fn with_editorconfig_files<'a>(
    lookups: impl IntoIterator<Item = (Lookup, Option<&'a Path>)>,
) -> Vec<Lookup> {
    let lookups: Vec<_> = lookups
        .into_iter()
        .map(|(lookup, dir)| {
            let reading = match &lookup {
                Lookup::Found(config) if config.editorconfig => dir,
                _ => None,
            };
            (lookup, reading)
        })
        .collect();
    let found = editorconfig::find_each(lookups.iter().filter_map(|(_, reading)| *reading));
    lookups
        .into_iter()
        .map(
            |(lookup, reading)| match (lookup, reading.map(|dir| &found[dir])) {
                (Lookup::Found(config), Some(Ok(editorconfig_files))) => Lookup::Found(Config {
                    editorconfig_files: editorconfig_files.clone(),
                    ..config
                }),
                (Lookup::Found(_), Some(Err(error))) => Lookup::Wrong(vec![error.clone()]),
                (lookup, _) => lookup,
            },
        )
        .collect()
}

fn nearest(starts: &[Result<PathBuf, LookupError>]) -> Vec<Lookup> {
    let found = find_each(starts.iter().flatten().map(PathBuf::as_path));
    let config_files: HashSet<&Path> = starts
        .iter()
        .flatten()
        .filter_map(|dir| found[dir.as_path()].as_ref().ok()?.as_deref())
        .collect();
    let loaded: HashMap<&Path, Result<Config, Vec<LookupError>>> = config_files
        .into_iter()
        .map(|file| (file, load_nearest(file)))
        .collect();
    let lookup = |dir: &Path| match &found[dir] {
        Err(error) => Lookup::Wrong(vec![error.clone()]),
        Ok(None) => Lookup::Found(Config::default()),
        Ok(Some(file)) => match &loaded[file.as_path()] {
            Ok(config) => Lookup::Found(config.clone()),
            Err(errors) => Lookup::Wrong(errors.clone()),
        },
    };
    starts
        .iter()
        .map(|start| match start {
            Ok(dir) => lookup(dir),
            Err(error) => Lookup::Unresolved(error.clone()),
        })
        .collect()
}

/// `file` is canonical.
fn search_start(file: &Path) -> PathBuf {
    file.parent().unwrap_or(file).to_path_buf()
}

/// Symlinks are resolved, since what a path names is where they lead.
/// Resolves `..` too, since `find` reads it as written.
pub fn canonical(path: &Path) -> Result<PathBuf, LookupError> {
    fs::canonicalize(path).map_err(|error| LookupError::new(path, error))
}

pub fn resolved(file: &Path) -> Result<PathBuf, LookupError> {
    if file.exists() {
        return canonical(file);
    }
    let name = file
        .file_name()
        .ok_or_else(|| LookupError::new(file, "names no file"))?;
    Ok(resolved_dir(file)?.join(name))
}

fn nearest_existing_dir(path: &Path) -> Result<PathBuf, LookupError> {
    // None on a drive of Windows that does not exist.
    resolved_dir(path)?
        .ancestors()
        .find(|dir| dir.is_dir())
        .map(Path::to_path_buf)
        .ok_or_else(|| LookupError::new(path, "no directory above it exists"))
}

/// A `..` after a directory that does not exist yet resolves as written, since it holds no symlink.
fn resolved_dir(path: &Path) -> Result<PathBuf, LookupError> {
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

pub fn load_given(path: &Path) -> Result<Config, Vec<LookupError>> {
    let config = load_file(path)?;
    // Not resolved unless a pattern needs it, since nuten may run in a directory that is removed.
    if !config.needs_root() {
        return Ok(config);
    }
    let root = fs::canonicalize(".").map_err(|error| {
        vec![LookupError::new(
            path,
            format!(
                "its patterns are relative to the current directory, which cannot be resolved: \
                 {error}"
            ),
        )]
    })?;
    Ok(Config {
        root: Some(root),
        ..config
    })
}

/// `path` is in a canonical directory.
fn load_nearest(path: &Path) -> Result<Config, Vec<LookupError>> {
    let config = load_file(path)?;
    Ok(Config {
        root: path.parent().map(Path::to_path_buf),
        ..config
    })
}

fn load_file(path: &Path) -> Result<Config, Vec<LookupError>> {
    let Some(format) = Format::of_path(path) else {
        let extensions: Vec<_> = Format::ALL
            .iter()
            .map(|format| format!("`.{}`", format.extension()))
            .collect();
        return Err(vec![LookupError::new(
            path,
            format!(
                "config file needs one of the extensions {}",
                extensions.join(", ")
            ),
        )]);
    };
    let text = read_regular_file(path).map_err(|error| vec![LookupError::new(path, error)])?;
    crate::load(&text, format).map_err(|errors| {
        located(
            &text,
            errors
                .iter()
                .map(|error| (error.range.start().into(), &error.kind)),
        )
        .into_iter()
        .map(|(line_column, kind)| LookupError {
            line_column: Some(line_column),
            ..LookupError::new(path, kind)
        })
        .collect()
    })
}

/// Not `fs::read_to_string` alone, which waits forever on a FIFO.
pub(crate) fn read_regular_file(path: &Path) -> io::Result<String> {
    if fs::metadata(path)?.is_file() {
        fs::read_to_string(path)
    } else {
        Err(io::Error::other("not a regular file"))
    }
}

#[cfg(test)]
mod tests {
    use std::{env, fs, path::PathBuf, process};

    use nuten_line_index::LineColumn;

    use super::{LookupError, load_given};

    #[test]
    fn lookup_error_reads_as_path_line_column_and_message() {
        let error = LookupError {
            path: PathBuf::from("nuten.nuon"),
            line_column: None,
            message: "e".to_string(),
        };
        let placed = LookupError {
            line_column: Some(LineColumn {
                line: 2,
                column: 3,
            }),
            ..error.clone()
        };
        assert_eq!(error.to_string(), "nuten.nuon: e");
        assert_eq!(placed.to_string(), "nuten.nuon:2:3: e");
    }

    #[test]
    fn load_given_places_each_error_of_config_text() {
        let path = env::temp_dir().join(format!("nuten-load-given-{}.nuon", process::id()));
        fs::write(&path, "{\n  a: 1}").unwrap();
        let errors = load_given(&path).unwrap_err();
        fs::remove_file(&path).ok();
        let expected = LookupError {
            path,
            line_column: Some(LineColumn {
                line: 2,
                column: 3,
            }),
            message: "unknown key `a`".to_string(),
        };
        assert_eq!(errors, [expected]);
    }
}
