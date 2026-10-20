use std::{
    collections::{BTreeMap, HashMap, HashSet},
    error::Error,
    ffi::{OsStr, OsString},
    fs::{self, FileType},
    io, iter,
    path::{Component, Path, PathBuf},
};

use nuten_config::{Lookup, LookupError, VCS_DIRS};

use crate::{
    gitignore::{self, DirIgnores, IgnorePatterns, UnreadFile},
    problem::Problem,
};

pub(crate) type Entry = Result<Candidate, Problem>;

#[derive(Debug)]
pub(crate) struct Candidate {
    pub(crate) path: PathBuf,
    pub(crate) canonical: Result<PathBuf, LookupError>,
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
    let global_excludes = if dir_lookups
        .values()
        .any(|lookup| matches!(lookup, Lookup::Found(config) if config.respect_gitignore))
    {
        gitignore::global_excludes().map_err(|unread| unread_problem(&unread))
    } else {
        Ok(None)
    };
    paths
        .iter()
        .flat_map(|path| match dir_lookups.get(path.as_path()) {
            None => vec![Ok(Candidate {
                path: path.clone(),
                canonical: nuten_config::canonical(path),
                given: true,
            })],
            Some(Lookup::Found(config)) if config.respect_gitignore => match &global_excludes {
                Ok(global_excludes) => walk(
                    path,
                    Some(IgnoreFiles {
                        global_excludes: global_excludes.as_ref(),
                    }),
                ),
                // Not walked, since what the global excludes file of git ignores is unknown.
                Err(problem) => vec![Err(problem.clone())],
            },
            Some(Lookup::Found(_)) => walk(path, None),
            // Not walked, since the config to walk it with is unknown.
            Some(lookup) => lookup
                .errors()
                .into_iter()
                .map(|error| Err(error.into()))
                .collect(),
        })
        .collect()
}

#[derive(Debug, Clone, Copy)]
struct IgnoreFiles<'a> {
    global_excludes: Option<&'a IgnorePatterns>,
}

struct Walk<'a> {
    root: &'a Path,
    canonical_root: Result<PathBuf, LookupError>,
    ignore_files: Option<IgnoreFiles<'a>>,
}

/// Walks no symlink, since a link could lead out of `dir` or back into it.
/// Walks no directory of version control, since it holds no code of the author's.
/// Hidden files are walked, since config of Nushell lives in them.
fn walk(dir: &Path, ignore_files: Option<IgnoreFiles<'_>>) -> Vec<Entry> {
    let walk = Walk {
        root: dir,
        // Resolved once, since the walk follows no symlink below it.
        canonical_root: nuten_config::canonical(dir),
        ignore_files,
    };
    if ignore_files.is_none() {
        return walk.dir(dir, &[], &[]);
    }
    // Without the directories above, only the ignore files below are read, as ripgrep does.
    let Ok(canonical_root) = &walk.canonical_root else {
        return walk.dir(dir, &[], &[]);
    };
    let names: Vec<&OsStr> = canonical_root
        .components()
        .filter_map(|component| match component {
            Component::Normal(name) => Some(name),
            _ => None,
        })
        .collect();
    match levels_above(canonical_root, &names) {
        Ok(levels) => walk.dir(
            dir,
            &path_prefix(&names),
            &levels.iter().collect::<Vec<_>>(),
        ),
        // Not walked, since an ignore file above `dir` covers all of it.
        Err(unread) => unread_problems(&unread),
    }
}

fn path_prefix(names: &[&OsStr]) -> Vec<u8> {
    names
        .iter()
        .flat_map(|name| [name.as_encoded_bytes(), b"/"])
        .flatten()
        .copied()
        .collect()
}

fn levels_above(
    canonical_root: &Path,
    names: &[&OsStr],
) -> Result<Vec<DirIgnores>, Vec<UnreadFile>> {
    let dirs: Vec<&Path> = canonical_root.ancestors().collect();
    let dirs: Vec<&Path> = dirs.into_iter().rev().collect();
    // With the root, which is no level above but may be the innermost repository.
    let is_repos: Vec<bool> = dirs
        .iter()
        .map(|dir| gitignore::is_repo(holds_on_disk(dir)))
        .collect();
    // Gitignore applies from the innermost repository down only.
    let innermost_repo = is_repos.iter().rposition(|&is_repo| is_repo);
    let prefix_lens = names.iter().scan(0, |len, name| {
        let prefix_len = *len;
        *len += name.len() + 1;
        Some(prefix_len)
    });
    let (levels, unread): (Vec<_>, Vec<_>) = dirs[..names.len()]
        .iter()
        .zip(&is_repos)
        .zip(prefix_lens)
        .enumerate()
        .map(|(depth, ((dir, &is_repo), prefix_len))| {
            let reads_gitignore = innermost_repo.is_some_and(|repo| depth >= repo);
            DirIgnores::read(
                dir,
                prefix_len,
                is_repo,
                reads_gitignore,
                holds_on_disk(dir),
            )
        })
        .partition(Result::is_ok);
    if unread.is_empty() {
        Ok(levels.into_iter().flatten().collect())
    } else {
        Err(unread
            .into_iter()
            .filter_map(Result::err)
            .flatten()
            .collect())
    }
}

fn holds_on_disk(dir: &Path) -> impl Fn(&str) -> bool + '_ {
    move |name| dir.join(name).symlink_metadata().is_ok()
}

impl Walk<'_> {
    fn dir(&self, dir: &Path, prefix: &[u8], above: &[&DirIgnores]) -> Vec<Entry> {
        let children = match children(dir) {
            Ok(children) => children,
            Err(error) => return vec![Err(Problem::in_file(&shown(dir), innermost(&error)))],
        };
        let own = match self.own_ignores(dir, prefix.len(), &children, above) {
            Ok(own) => own,
            // Not walked, since what it ignores below is unknown.
            Err(unread) => return unread_problems(&unread),
        };
        let levels: Vec<&DirIgnores> = above.iter().copied().chain(&own).collect();
        children
            .iter()
            .filter(|&(name, &file_type)| is_walked(name, file_type))
            .flat_map(|(name, &file_type)| {
                self.child(dir, prefix, name, file_type.is_dir(), &levels)
            })
            .collect()
    }

    fn own_ignores(
        &self,
        dir: &Path,
        prefix_len: usize,
        children: &BTreeMap<OsString, FileType>,
        above: &[&DirIgnores],
    ) -> Result<Option<DirIgnores>, Vec<UnreadFile>> {
        if self.ignore_files.is_none() {
            return Ok(None);
        }
        let holds = |name: &str| children.contains_key(OsStr::new(name));
        let is_repo = gitignore::is_repo(holds);
        let reads_gitignore = is_repo || above.iter().any(|level| level.is_repo());
        DirIgnores::read(dir, prefix_len, is_repo, reads_gitignore, holds).map(Some)
    }

    fn child(
        &self,
        dir: &Path,
        prefix: &[u8],
        name: &OsStr,
        is_dir: bool,
        levels: &[&DirIgnores],
    ) -> Vec<Entry> {
        let path_bytes = [prefix, name.as_encoded_bytes()].concat();
        if let Some(ignore_files) = self.ignore_files
            && gitignore::ignores(levels, ignore_files.global_excludes, &path_bytes, is_dir)
        {
            return Vec::new();
        }
        let path = dir.join(name);
        if is_dir {
            return self.dir(&path, &[&path_bytes[..], b"/"].concat(), levels);
        }
        let below = path
            .strip_prefix(self.root)
            .expect("the walk yields paths under its root");
        vec![Ok(Candidate {
            path: shown(&path),
            canonical: self.canonical_root.clone().map(|root| root.join(below)),
            given: false,
        })]
    }
}

fn is_walked(name: &OsStr, file_type: FileType) -> bool {
    file_type.is_file() || file_type.is_dir() && !VCS_DIRS.iter().any(|vcs| name == *vcs)
}

/// Sorted, so that files come in one order wherever they are walked.
/// Types are those of the entries themselves, so that a symlink is seen as one.
fn children(dir: &Path) -> io::Result<BTreeMap<OsString, FileType>> {
    fs::read_dir(dir)?
        .map(|entry| {
            let entry = entry?;
            Ok((entry.file_name(), entry.file_type()?))
        })
        .collect()
}

fn unread_problems(unread: &[UnreadFile]) -> Vec<Entry> {
    unread
        .iter()
        .map(|unread| Err(unread_problem(unread)))
        .collect()
}

/// Canonical, so that one ignore file is named one way from whichever directory it is met.
fn unread_problem(unread: &UnreadFile) -> Problem {
    let path = fs::canonicalize(&unread.path).unwrap_or_else(|_| unread.path.clone());
    Problem::in_file(&path, innermost(&unread.error))
}

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

fn innermost(error: &io::Error) -> &io::Error {
    let outermost: &(dyn Error + 'static) = error;
    iter::successors(Some(outermost), |&error| error.source())
        .filter_map(|error| error.downcast_ref::<io::Error>())
        .last()
        .unwrap_or(error)
}

/// One of `excluded` under another name is dropped too, since `exclude` leaves that file alone.
pub(crate) fn distinct<T>(
    entries: Vec<Result<T, Problem>>,
    excluded: &[PathBuf],
    path_of: impl Fn(&T) -> &PathBuf,
) -> Vec<Result<T, Problem>> {
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
    /// Kept, so that reading the file for formatting reports why its identity cannot be read.
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
