//! Where the config file for a directory is.

use std::{
    error, fmt, fs, io,
    path::{Path, PathBuf},
};

/// The names a config file may have, in the order they win in one directory.
pub const FILE_NAMES: [&str; 3] = ["nuten.nuon", "nuten.jsonc", "nuten.json"];

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
/// `dir` is taken as it is written, so a `..` in it is an ancestor like any other.
pub fn find(dir: &Path) -> Result<Option<PathBuf>, FindError> {
    dir.ancestors()
        .flat_map(|ancestor| FILE_NAMES.map(|name| ancestor.join(name)))
        .find_map(|path| match fs::symlink_metadata(&path) {
            Ok(_) => Some(Ok(path)),
            Err(source) if source.kind() == io::ErrorKind::NotFound => None,
            Err(source) => Some(Err(FindError {
                path,
                source,
            })),
        })
        .transpose()
}

#[cfg(test)]
mod tests {
    use super::FILE_NAMES;
    use crate::Format;

    #[test]
    fn file_names_take_extension_of_each_format_in_order() {
        let names = Format::ALL.map(|format| format!("nuten.{}", format.extension()));
        assert_eq!(names, FILE_NAMES);
    }
}
