//! The files to format.

use std::{
    collections::HashSet,
    fs, io,
    path::{Path, PathBuf},
};

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
