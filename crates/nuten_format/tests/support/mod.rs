//! Shared by the tests that run nuten over many files.

use std::{
    fs,
    panic::{self, UnwindSafe},
    path::{Path, PathBuf},
};

/// Sorted, since the order `read_dir` gives depends on the filesystem.
/// Without sorting, the reports would differ from one machine to another.
pub(crate) fn files_below(dir: &Path, descends: &impl Fn(&Path) -> bool) -> Vec<PathBuf> {
    let mut entries: Vec<_> = fs::read_dir(dir)
        .unwrap_or_else(|error| panic!("{}: {error}", dir.display()))
        .map(|entry| entry.unwrap_or_else(|error| panic!("an entry of {}: {error}", dir.display())))
        .collect();
    entries.sort_by_key(fs::DirEntry::path);
    entries
        .into_iter()
        .flat_map(|entry| {
            let path = entry.path();
            let is_dir = entry
                .file_type()
                .unwrap_or_else(|error| panic!("{}: {error}", path.display()))
                .is_dir();
            if !is_dir {
                vec![path]
            } else if descends(&path) {
                files_below(&path, descends)
            } else {
                Vec::new()
            }
        })
        .collect()
}

/// What `check` gives, or the message of the panic it raised.
/// A panic is a bug in nuten itself; catching it keeps the rest of the files checked.
pub(crate) fn caught<T>(check: impl FnOnce() -> T + UnwindSafe) -> Result<T, String> {
    panic::catch_unwind(check).map_err(|payload| {
        payload
            .downcast_ref::<String>()
            .map(String::as_str)
            .or_else(|| payload.downcast_ref::<&str>().copied())
            .unwrap_or("a panic")
            .to_owned()
    })
}
