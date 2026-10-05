use std::{
    collections::HashMap,
    error, fmt, fs, io,
    path::{Path, PathBuf},
};

use crate::LookupError;

pub const FILE_NAMES: [&str; 3] = ["nuten.nuon", "nuten.jsonc", "nuten.json"];

pub const VCS_DIRS: [&str; 5] = [".git", ".hg", ".svn", ".bzr", ".jj"];

#[derive(Debug)]
pub struct FindError {
    pub path: PathBuf,
    pub source: io::Error,
}

impl fmt::Display for FindError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.path.display(), self.source)
    }
}

impl From<FindError> for LookupError {
    fn from(error: FindError) -> LookupError {
        LookupError::new(&error.path, error.source)
    }
}

impl error::Error for FindError {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        Some(&self.source)
    }
}

/// `dir` is taken as it is written, so a `..` in it is an ancestor like any other.
pub fn find(dir: &Path) -> Result<Option<PathBuf>, FindError> {
    // One buffer, since the search probes every name in every ancestor and each path allocates.
    let mut path = PathBuf::new();
    for ancestor in dir.ancestors() {
        if let Step::Ends(found) = step(&mut path, ancestor)? {
            return Ok(found);
        }
    }
    Ok(None)
}

/// An ancestor they share is probed once, since a walk looks for the config of every file.
pub(crate) fn find_each<'a>(
    dirs: impl IntoIterator<Item = &'a Path>,
) -> HashMap<&'a Path, Result<Option<PathBuf>, LookupError>> {
    let mut path = PathBuf::new();
    let mut found = HashMap::new();
    for dir in dirs {
        let (passed, result) = search(&mut path, dir, &found);
        for dir in passed {
            found.insert(dir, result.clone());
        }
    }
    found
}

fn search<'a>(
    path: &mut PathBuf,
    dir: &'a Path,
    found: &HashMap<&Path, Result<Option<PathBuf>, LookupError>>,
) -> (Vec<&'a Path>, Result<Option<PathBuf>, LookupError>) {
    let mut passed = Vec::new();
    for ancestor in dir.ancestors() {
        if let Some(known) = found.get(ancestor) {
            return (passed, known.clone());
        }
        passed.push(ancestor);
        match step(path, ancestor) {
            Ok(Step::GoesOn) => {},
            Ok(Step::Ends(file)) => return (passed, Ok(file)),
            Err(error) => return (passed, Err(error.into())),
        }
    }
    (passed, Ok(None))
}

enum Step {
    GoesOn,
    Ends(Option<PathBuf>),
}

fn step(path: &mut PathBuf, dir: &Path) -> Result<Step, FindError> {
    for name in FILE_NAMES {
        if exists(path, dir, name)? {
            return Ok(Step::Ends(Some(path.clone())));
        }
    }
    // A config file above the root would format the repository by where it is cloned.
    if holds_vcs_dir_in(path, dir)? {
        return Ok(Step::Ends(None));
    }
    Ok(Step::GoesOn)
}

pub(crate) fn holds_vcs_dir(dir: &Path) -> Result<bool, FindError> {
    holds_vcs_dir_in(&mut PathBuf::new(), dir)
}

fn holds_vcs_dir_in(path: &mut PathBuf, dir: &Path) -> Result<bool, FindError> {
    for name in VCS_DIRS {
        if exists(path, dir, name)? {
            return Ok(true);
        }
    }
    Ok(false)
}

fn exists(path: &mut PathBuf, dir: &Path, name: &str) -> Result<bool, FindError> {
    path.as_mut_os_string().clear();
    path.push(dir);
    path.push(name);
    match fs::symlink_metadata(&path) {
        Ok(_) => Ok(true),
        Err(source) if source.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(source) => Err(FindError {
            path: path.clone(),
            source,
        }),
    }
}

#[cfg(test)]
mod tests {
    use std::{
        env, fs, io,
        path::{Path, PathBuf},
    };

    use super::{FILE_NAMES, FindError, exists, find, find_each};
    use crate::{Format, LookupError};

    #[test]
    fn file_names_take_extension_of_each_format_in_order() {
        let names = Format::ALL.map(|format| format!("nuten.{}", format.extension()));
        assert_eq!(names, FILE_NAMES);
    }

    struct TempDir(PathBuf);

    impl Drop for TempDir {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.0).ok();
        }
    }

    #[test]
    fn find_each_finds_what_find_does_for_each_directory_in_any_order() {
        let root = TempDir(env::temp_dir().join(format!("nuten-find-each-{}", std::process::id())));
        for dir in ["a/b/c", "v/.git", "v/w/x", "y/z"] {
            fs::create_dir_all(root.0.join(dir)).unwrap();
        }
        fs::write(root.0.join("a/nuten.nuon"), "{}").unwrap();
        fs::write(root.0.join("a/b/c/nuten.json"), "{}").unwrap();
        let dirs: Vec<PathBuf> = ["a/b/c", "a/b", "a", "v/w/x", "v", "v/w", "y/z", "y"]
            .iter()
            .map(|dir| root.0.join(dir))
            .collect();
        let found = find_each(dirs.iter().map(PathBuf::as_path));
        for dir in &dirs {
            let expected = find(dir).map_err(LookupError::from);
            assert_eq!(found[dir.as_path()], expected, "{}", dir.display());
        }
    }

    #[test]
    fn exists_leaves_path_as_relative_dir_joined_with_name_after_earlier_probe() {
        let mut path = PathBuf::new();
        exists(&mut path, Path::new("missing/a"), "b").unwrap();
        exists(&mut path, Path::new("missing/c"), "d").unwrap();
        assert_eq!(path, Path::new("missing/c/d"));
    }

    #[test]
    fn lookup_error_of_find_error_names_its_path() {
        let error = FindError {
            path: PathBuf::from("a/nuten.nuon"),
            source: io::Error::other("e"),
        };
        assert_eq!(
            LookupError::from(error),
            LookupError::new(Path::new("a/nuten.nuon"), "e")
        );
    }
}
