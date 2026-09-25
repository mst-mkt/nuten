//! Looks for config files in a temporary directory tree.

use std::{
    env, fs, io,
    path::{Path, PathBuf},
    sync::atomic::{AtomicUsize, Ordering},
};

use nuten_config::{FILE_NAMES, find};

/// A fresh directory for each test, removed when dropped.
/// No config file is above it.
struct TempDir(PathBuf);

impl TempDir {
    fn new() -> TempDir {
        static COUNT: AtomicUsize = AtomicUsize::new(0);
        let count = COUNT.fetch_add(1, Ordering::Relaxed);
        let dir = env::temp_dir().join(format!("nuten-find-{}-{count}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let dir = TempDir(dir);
        let above = find(&dir.0).unwrap();
        assert_eq!(above, None, "a config file above the temporary directory");
        dir
    }

    fn path(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }

    fn dir(&self, name: &str) -> PathBuf {
        let path = self.path(name);
        fs::create_dir_all(&path).unwrap();
        path
    }

    fn file(&self, name: &str) -> PathBuf {
        let path = self.path(name);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, "{}").unwrap();
        path
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).ok();
    }
}

fn found(dir: &Path) -> Option<PathBuf> {
    find(dir).unwrap()
}

#[test]
fn file_in_directory_is_found() {
    for name in FILE_NAMES {
        let dir = TempDir::new();
        let path = dir.file(name);
        assert_eq!(found(&dir.0), Some(path));
    }
}

#[test]
fn nuon_wins_over_jsonc_and_json() {
    let dir = TempDir::new();
    let nuon = dir.file("nuten.nuon");
    dir.file("nuten.jsonc");
    dir.file("nuten.json");
    assert_eq!(found(&dir.0), Some(nuon));
}

#[test]
fn jsonc_wins_over_json() {
    let dir = TempDir::new();
    let jsonc = dir.file("nuten.jsonc");
    dir.file("nuten.json");
    assert_eq!(found(&dir.0), Some(jsonc));
}

#[test]
fn nearest_directory_wins_over_name() {
    let dir = TempDir::new();
    dir.file("nuten.nuon");
    let json = dir.file("a/nuten.json");
    assert_eq!(found(&dir.dir("a/b")), Some(json));
}

#[test]
fn file_in_ancestor_is_found() {
    let dir = TempDir::new();
    let path = dir.file("nuten.jsonc");
    assert_eq!(found(&dir.dir("a/b/c")), Some(path));
}

#[test]
fn file_in_descendant_is_not_found() {
    let dir = TempDir::new();
    dir.file("a/nuten.nuon");
    assert_eq!(found(&dir.0), None);
}

#[test]
fn file_above_repository_root_is_not_found() {
    for name in [".git", ".hg", ".svn", ".bzr", ".jj"] {
        let dir = TempDir::new();
        dir.file("nuten.nuon");
        dir.dir(&format!("a/{name}"));
        assert_eq!(found(&dir.dir("a/b")), None, "{name}");
    }
}

#[test]
fn file_above_repository_root_is_not_found_when_dot_git_is_file() {
    let dir = TempDir::new();
    dir.file("nuten.nuon");
    dir.file("a/.git");
    assert_eq!(found(&dir.dir("a/b")), None);
}

#[cfg(unix)]
#[test]
fn file_above_repository_root_is_not_found_when_dot_git_is_dangling_symlink() {
    let dir = TempDir::new();
    dir.file("nuten.nuon");
    std::os::unix::fs::symlink(dir.path("missing"), dir.dir("a").join(".git")).unwrap();
    assert_eq!(found(&dir.dir("a/b")), None);
}

#[test]
fn file_above_repository_root_is_not_found_from_repository_root() {
    let dir = TempDir::new();
    dir.file("nuten.nuon");
    dir.dir("a/.git");
    assert_eq!(found(&dir.path("a")), None);
}

#[test]
fn file_at_repository_root_is_found() {
    let dir = TempDir::new();
    dir.dir("a/.git");
    let path = dir.file("a/nuten.nuon");
    assert_eq!(found(&dir.dir("a/b")), Some(path));
}

#[test]
fn file_above_nested_repository_root_is_not_found() {
    let dir = TempDir::new();
    dir.dir(".git");
    dir.file("nuten.nuon");
    dir.file("a/.git");
    assert_eq!(found(&dir.dir("a/b")), None);
}

#[test]
fn file_above_directory_named_git_without_dot_is_found() {
    let dir = TempDir::new();
    let path = dir.file("nuten.nuon");
    dir.dir("a/git");
    assert_eq!(found(&dir.dir("a")), Some(path));
}

#[test]
fn dot_dot_in_directory_is_taken_as_written() {
    let dir = TempDir::new();
    let path = dir.file("a/nuten.nuon");
    dir.dir("b");
    assert_eq!(found(&dir.path("a/../b/..")), Some(path));
}

#[test]
fn directory_of_config_name_is_found() {
    let dir = TempDir::new();
    let path = dir.dir("nuten.nuon");
    dir.file("nuten.json");
    assert_eq!(found(&dir.0), Some(path));
}

#[cfg(unix)]
#[test]
fn dangling_symlink_of_config_name_is_found() {
    let dir = TempDir::new();
    let path = dir.path("nuten.nuon");
    std::os::unix::fs::symlink(dir.path("missing"), &path).unwrap();
    dir.file("nuten.json");
    assert_eq!(found(&dir.0), Some(path));
}

#[cfg(unix)]
#[test]
fn unsearchable_directory_is_reported_with_path() {
    use std::os::unix::fs::PermissionsExt;

    let dir = TempDir::new();
    let sub = dir.dir("a");
    fs::set_permissions(&sub, fs::Permissions::from_mode(0o000)).unwrap();
    let result = find(&sub);
    let searchable = fs::symlink_metadata(sub.join("nuten.nuon"))
        .is_err_and(|error| error.kind() == io::ErrorKind::NotFound);
    fs::set_permissions(&sub, fs::Permissions::from_mode(0o755)).unwrap();
    // Root searches a directory without permission, so the test proves nothing then.
    if searchable {
        return;
    }
    let error = result.unwrap_err();
    assert_eq!(error.path, sub.join(FILE_NAMES[0]));
    assert_eq!(error.source.kind(), io::ErrorKind::PermissionDenied);
}
