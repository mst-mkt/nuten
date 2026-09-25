//! Where the config file for a directory is.

use std::{
    error, fmt, fs, io,
    path::{Path, PathBuf},
};

/// The names a config file may have, in the order they win in one directory.
pub const FILE_NAMES: [&str; 3] = ["nuten.nuon", "nuten.jsonc", "nuten.json"];

pub const VCS_DIRS: [&str; 5] = [".git", ".hg", ".svn", ".bzr", ".jj"];

#[derive(Debug)]
pub struct FindError {
    /// The path whose existence could not be told.
    pub path: PathBuf,
    pub source: io::Error,
}

impl fmt::Display for FindError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.path.display(), self.source)
    }
}

impl error::Error for FindError {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        Some(&self.source)
    }
}

/// The config file in `dir`, or else in the nearest of its ancestors that holds one.
/// The search goes no higher than the root of the repository `dir` is in.
/// `dir` is taken as it is written, so a `..` in it is an ancestor like any other.
pub fn find(dir: &Path) -> Result<Option<PathBuf>, FindError> {
    // One buffer, since the search probes every name in every ancestor and each path allocates.
    let mut path = PathBuf::new();
    for ancestor in dir.ancestors() {
        for name in FILE_NAMES {
            if exists(&mut path, ancestor, name)? {
                return Ok(Some(path));
            }
        }
        // A config file above the root would format the repository by where it is cloned.
        for name in VCS_DIRS {
            if exists(&mut path, ancestor, name)? {
                return Ok(None);
            }
        }
    }
    Ok(None)
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
    use std::path::{Path, PathBuf};

    use super::{FILE_NAMES, exists};
    use crate::Format;

    #[test]
    fn file_names_take_extension_of_each_format_in_order() {
        let names = Format::ALL.map(|format| format!("nuten.{}", format.extension()));
        assert_eq!(names, FILE_NAMES);
    }

    #[test]
    fn exists_leaves_path_as_relative_dir_joined_with_name_after_earlier_probe() {
        let mut path = PathBuf::new();
        exists(&mut path, Path::new("missing/a"), "b").unwrap();
        exists(&mut path, Path::new("missing/c"), "d").unwrap();
        assert_eq!(path, Path::new("missing/c/d"));
    }
}
