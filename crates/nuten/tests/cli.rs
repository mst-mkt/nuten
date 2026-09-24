//! Runs the binary on files in a temporary directory, and with its stdout closed or full.

use std::{
    env,
    ffi::OsStr,
    fs, io,
    path::{Path, PathBuf},
    process::{Command, Output, Stdio},
    sync::atomic::{AtomicUsize, Ordering},
    thread,
    time::{Duration, Instant},
};

/// A fresh directory for each test, removed when dropped.
/// Its path has no symlinks, since the paths of config files are reported with symlinks resolved.
/// No config file is above it.
struct TempDir(PathBuf);

impl TempDir {
    fn new() -> TempDir {
        static COUNT: AtomicUsize = AtomicUsize::new(0);
        let count = COUNT.fetch_add(1, Ordering::Relaxed);
        let dir = env::temp_dir().join(format!("nuten-cli-{}-{count}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let dir = TempDir(fs::canonicalize(&dir).unwrap());
        let above = nuten_config::find(&dir.0).unwrap();
        assert_eq!(above, None, "a config file above the temporary directory");
        dir
    }

    fn path(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }

    /// Creates the directory `name`, with its parents.
    fn dir(&self, name: &str) -> PathBuf {
        let path = self.path(name);
        fs::create_dir_all(&path).unwrap();
        path
    }

    /// Writes the file `name`, creating the directories it is in.
    fn file(&self, name: &str, contents: &str) -> PathBuf {
        let path = self.path(name);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, contents).unwrap();
        path
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).ok();
    }
}

fn nuten(args: &[&OsStr]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_nuten"))
        .args(args)
        .output()
        .unwrap()
}

fn format(paths: &[&Path]) -> Output {
    let args: Vec<_> = [OsStr::new("format")]
        .into_iter()
        .chain(paths.iter().map(|path| path.as_os_str()))
        .collect();
    nuten(&args)
}

/// Runs `nuten format` with `args` from `dir`, so that relative paths are in `dir`.
fn format_in(dir: &TempDir, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_nuten"))
        .arg("format")
        .args(args)
        .current_dir(&dir.0)
        .output()
        .unwrap()
}

/// A block, which each indent width formats differently.
const BLOCK: &str = "if true {\nls\n}";

fn read(path: &Path) -> String {
    fs::read_to_string(path).unwrap()
}

fn stderr(output: &Output) -> String {
    String::from_utf8(output.stderr.clone()).unwrap()
}

/// The line the binary reports for `error` at `path`, when it meets the same error.
fn io_error_line(path: &Path, error: &io::Error) -> String {
    format!("{}: {error}\n", path.display())
}

#[test]
fn format_rewrites_each_file_in_place() {
    let dir = TempDir::new();
    let a = dir.file("a.nu", "ls|first");
    let b = dir.file("b.nu", "echo   a\n\n\n");
    let output = format(&[&a, &b]);
    assert_eq!(output.status.code(), Some(0));
    assert!(output.stdout.is_empty());
    assert!(output.stderr.is_empty());
    assert_eq!(read(&a), "ls | first\n");
    assert_eq!(read(&b), "echo a\n");
}

#[test]
fn format_leaves_formatted_file_alone() {
    let dir = TempDir::new();
    let path = dir.file("a.nu", "ls | first\n");
    // Writing would fail.
    let mut permissions = fs::metadata(&path).unwrap().permissions();
    permissions.set_readonly(true);
    fs::set_permissions(&path, permissions).unwrap();
    let output = format(&[&path]);
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    assert!(output.stderr.is_empty());
}

#[test]
fn format_reports_syntax_error_position_and_continues() {
    let dir = TempDir::new();
    let bad = dir.file("bad.nu", "ls\n\n  ls || x\n");
    let good = dir.file("good.nu", "ls|first");
    let output = format(&[&bad, &good]);
    assert_eq!(output.status.code(), Some(2));
    let stderr = stderr(&output);
    assert_eq!(stderr.lines().count(), 1, "{stderr}");
    assert!(
        stderr.starts_with(&format!("{}:3:6: ", bad.display())),
        "{stderr}"
    );
    assert_eq!(read(&bad), "ls\n\n  ls || x\n");
    assert_eq!(read(&good), "ls | first\n");
}

#[test]
fn format_reports_unreadable_file() {
    let dir = TempDir::new();
    let missing = dir.path("missing.nu");
    let output = format(&[&missing]);
    assert_eq!(output.status.code(), Some(2));
    assert!(stderr(&output).starts_with(&format!("{}: ", missing.display())));
}

#[cfg(unix)]
#[test]
fn format_reports_fifo_without_waiting_and_continues() {
    let dir = TempDir::new();
    let fifo_path = dir.path("fifo.nu");
    fifo(&fifo_path);
    let good = dir.file("good.nu", "ls|first");
    let output = format_in_time(&[&fifo_path, &good]);
    assert_eq!(output.status.code(), Some(2));
    assert_eq!(
        stderr(&output),
        format!("{}: not a regular file\n", fifo_path.display())
    );
    assert_eq!(read(&good), "ls | first\n");
}

#[test]
fn format_wants_at_least_one_file() {
    let output = format(&[]);
    assert_eq!(output.status.code(), Some(2));
    assert!(!output.stderr.is_empty());
}

#[test]
fn format_rejects_unknown_flag() {
    let output = format(&[Path::new("--bogus")]);
    assert_eq!(output.status.code(), Some(2));
    let stderr = stderr(&output);
    assert!(stderr.contains("--bogus"), "{stderr}");
    assert!(!stderr.contains("No such file"), "{stderr}");
}

#[test]
fn format_takes_file_named_like_subcommand() {
    let dir = TempDir::new();
    let path = dir.file("format", "ls|first");
    let output = format(&[&path]);
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    assert_eq!(read(&path), "ls | first\n");
}

#[test]
fn format_takes_config_file_in_directory_of_file() {
    let dir = TempDir::new();
    dir.file("nuten.nuon", "{indent_width: 2}");
    let path = dir.file("a.nu", BLOCK);
    let output = format(&[&path]);
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    assert_eq!(read(&path), "if true {\n  ls\n}\n");
}

#[test]
fn format_takes_config_file_in_ancestor_directory() {
    let dir = TempDir::new();
    dir.file("nuten.jsonc", "// two\n{\"indent_width\": 2,}");
    let path = dir.file("a/b/a.nu", BLOCK);
    let output = format(&[&path]);
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    assert_eq!(read(&path), "if true {\n  ls\n}\n");
}

#[test]
fn format_takes_nearest_config_file_for_each_file() {
    let dir = TempDir::new();
    dir.file("nuten.json", r#"{"indent_width": 2}"#);
    dir.file("a/nuten.json", r#"{"indent_width": 3}"#);
    let top = dir.file("a.nu", BLOCK);
    let inner = dir.file("a/a.nu", BLOCK);
    let bare = dir.file("b/a.nu", BLOCK);
    let output = format(&[&top, &inner, &bare]);
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    assert_eq!(read(&top), "if true {\n  ls\n}\n");
    assert_eq!(read(&inner), "if true {\n   ls\n}\n");
    assert_eq!(read(&bare), "if true {\n  ls\n}\n");
}

#[test]
fn format_takes_nuon_over_jsonc_and_json_in_same_directory() {
    let dir = TempDir::new();
    dir.file("nuten.nuon", "{indent_width: 2}");
    dir.file("nuten.jsonc", r#"{"indent_width": 3}"#);
    dir.file("nuten.json", r#"{"indent_width": 5}"#);
    let path = dir.file("a.nu", BLOCK);
    let output = format(&[&path]);
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    assert_eq!(read(&path), "if true {\n  ls\n}\n");
}

#[test]
fn format_takes_defaults_when_config_file_is_only_in_subdirectory() {
    let dir = TempDir::new();
    dir.file("a/nuten.nuon", "{indent_width: 2}");
    let path = dir.file("a.nu", BLOCK);
    let output = format(&[&path]);
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    assert_eq!(read(&path), "if true {\n    ls\n}\n");
}

#[test]
fn format_looks_for_config_file_from_current_directory_for_bare_file_name() {
    let dir = TempDir::new();
    dir.file("nuten.nuon", "{indent_width: 2}");
    let path = dir.file("a.nu", BLOCK);
    let output = format_in(&dir, &["a.nu"]);
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    assert_eq!(read(&path), "if true {\n  ls\n}\n");
}

#[test]
fn format_looks_for_config_file_where_dot_dot_leads() {
    let dir = TempDir::new();
    dir.file("a/nuten.nuon", "{indent_width: 2}");
    let path = dir.file("a.nu", BLOCK);
    let output = format_in(&dir, &["a/../a.nu"]);
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    assert_eq!(read(&path), "if true {\n    ls\n}\n");
}

#[cfg(unix)]
#[test]
fn format_looks_for_config_file_where_symlinked_directory_leads() {
    let dir = TempDir::new();
    dir.file("a/nuten.nuon", "{indent_width: 2}");
    let path = dir.file("a/real/a.nu", BLOCK);
    std::os::unix::fs::symlink(dir.path("a/real"), dir.dir("b").join("link")).unwrap();
    let output = format_in(&dir, &["b/link/a.nu"]);
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    assert_eq!(read(&path), "if true {\n  ls\n}\n");
}

#[cfg(unix)]
#[test]
fn format_looks_for_config_file_where_symlinked_file_leads() {
    let dir = TempDir::new();
    dir.file("a/nuten.nuon", "{indent_width: 2}");
    dir.file("b/nuten.nuon", "{indent_width: 3}");
    let path = dir.file("a/a.nu", BLOCK);
    std::os::unix::fs::symlink(&path, dir.path("b/link.nu")).unwrap();
    let output = format_in(&dir, &["b/link.nu"]);
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    assert_eq!(read(&path), "if true {\n  ls\n}\n");
}

#[test]
fn format_takes_config_for_other_files_when_one_has_syntax_error() {
    let dir = TempDir::new();
    dir.file("nuten.nuon", "{indent_width: 2}");
    let bad = dir.file("bad.nu", "ls ||");
    let good = dir.file("good.nu", BLOCK);
    let output = format(&[&bad, &good]);
    assert_eq!(output.status.code(), Some(2));
    assert_eq!(read(&bad), "ls ||");
    assert_eq!(read(&good), "if true {\n  ls\n}\n");
}

#[test]
fn format_writes_nothing_when_any_config_file_is_wrong() {
    let dir = TempDir::new();
    dir.file("a/nuten.json", "{\n  \"indent_width\": 0\n}");
    let wrong = dir.file("a/a.nu", "ls|first");
    let right = dir.file("b/a.nu", "ls|first");
    let output = format(&[&right, &wrong]);
    assert_eq!(output.status.code(), Some(2));
    assert_eq!(
        stderr(&output),
        format!(
            "{}:2:19: `indent_width` takes an integer from 1 to 255\n",
            dir.path("a/nuten.json").display()
        )
    );
    assert_eq!(read(&wrong), "ls|first");
    assert_eq!(read(&right), "ls|first");
}

#[test]
fn format_reports_every_error_in_config_file() {
    let dir = TempDir::new();
    let config = dir.file("nuten.nuon", "{indent_width: 0, a: 1}");
    let path = dir.file("a.nu", "ls|first");
    let output = format(&[&path]);
    assert_eq!(output.status.code(), Some(2));
    assert_eq!(
        stderr(&output),
        format!(
            "{0}:1:16: `indent_width` takes an integer from 1 to 255\n\
             {0}:1:19: unknown key `a`\n",
            config.display()
        )
    );
}

#[test]
fn format_reports_config_error_line_in_crlf_file() {
    let dir = TempDir::new();
    let config = dir.file("nuten.json", "{\r\n  \"indent_width\": 0\r\n}");
    let path = dir.file("a.nu", "ls|first");
    let output = format(&[&path]);
    assert_eq!(output.status.code(), Some(2));
    assert_eq!(
        stderr(&output),
        format!(
            "{}:2:19: `indent_width` takes an integer from 1 to 255\n",
            config.display()
        )
    );
}

#[test]
fn format_reports_each_wrong_config_file_once() {
    let dir = TempDir::new();
    let nuon = dir.file("a/nuten.nuon", "{a: 1}");
    let json = dir.file("b/nuten.json", "{");
    let paths = ["a/x.nu", "b/x.nu", "a/y.nu", "b/c/x.nu"].map(|name| dir.file(name, "ls"));
    let output = format(&paths.each_ref().map(PathBuf::as_path));
    assert_eq!(output.status.code(), Some(2));
    assert_eq!(
        stderr(&output),
        format!(
            "{}:1:2: unknown key `a`\n{}:1:1: Unterminated object\n",
            nuon.display(),
            json.display()
        )
    );
}

#[test]
fn format_does_not_fall_back_from_wrong_config_file_to_next_name() {
    let dir = TempDir::new();
    let nuon = dir.file("nuten.nuon", "{indent_width: 2");
    dir.file("nuten.json", r#"{"indent_width": 2}"#);
    let path = dir.file("a.nu", BLOCK);
    let output = format(&[&path]);
    assert_eq!(output.status.code(), Some(2));
    assert_eq!(
        stderr(&output),
        format!("{}:1:1: unclosed quote or bracket\n", nuon.display())
    );
    assert_eq!(read(&path), BLOCK);
}

#[cfg(unix)]
#[test]
fn format_does_not_fall_back_from_dangling_symlink_config_file() {
    let dir = TempDir::new();
    let nuon = dir.path("nuten.nuon");
    std::os::unix::fs::symlink(dir.path("missing"), &nuon).unwrap();
    dir.file("nuten.json", r#"{"indent_width": 2}"#);
    let path = dir.file("a.nu", BLOCK);
    let output = format(&[&path]);
    assert_eq!(output.status.code(), Some(2));
    let error = fs::read(&nuon).unwrap_err();
    assert_eq!(stderr(&output), io_error_line(&nuon, &error));
    assert_eq!(read(&path), BLOCK);
}

#[test]
fn format_writes_nothing_when_config_file_is_directory() {
    let dir = TempDir::new();
    let nuon = dir.dir("nuten.nuon");
    let path = dir.file("a.nu", "ls|first");
    let output = format(&[&path]);
    assert_eq!(output.status.code(), Some(2));
    assert_eq!(
        stderr(&output),
        format!("{}: not a regular file\n", nuon.display())
    );
    assert_eq!(read(&path), "ls|first");
}

/// Makes a FIFO at `path`.
#[cfg(unix)]
fn fifo(path: &Path) {
    let status = Command::new("mkfifo").arg(path).status().unwrap();
    assert!(status.success());
}

/// Runs `nuten format` on `paths`, and fails when it has not ended in 10 seconds.
/// Not `format`, which would hang along with the binary if it read a FIFO.
#[cfg(unix)]
fn format_in_time(paths: &[&Path]) -> Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_nuten"))
        .arg("format")
        .args(paths)
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(10);
    while child.try_wait().unwrap().is_none() {
        if Instant::now() > deadline {
            child.kill().unwrap();
            panic!("the binary waits on a FIFO");
        }
        thread::sleep(Duration::from_millis(10));
    }
    child.wait_with_output().unwrap()
}

#[cfg(unix)]
#[test]
fn format_writes_nothing_without_waiting_when_config_file_is_fifo() {
    let dir = TempDir::new();
    let nuon = dir.path("nuten.nuon");
    fifo(&nuon);
    let path = dir.file("a.nu", "ls|first");
    let output = format_in_time(&[&path]);
    assert_eq!(output.status.code(), Some(2));
    assert_eq!(
        stderr(&output),
        format!("{}: not a regular file\n", nuon.display())
    );
    assert_eq!(read(&path), "ls|first");
}

#[cfg(target_os = "linux")]
#[test]
fn format_writes_nothing_when_existence_of_config_file_cannot_be_told() {
    const PATH_MAX: usize = 4096;
    let dir = TempDir::new();
    // A directory whose path leaves room for `/a.nu` but not for `/nuten.nuon`.
    let deep_length = PATH_MAX - 8;
    // Components of 99 bytes, since one longer than NAME_MAX cannot be created.
    let relative: String = (0..deep_length - dir.0.as_os_str().len() - 1)
        .map(|index| if index % 100 == 99 { '/' } else { 'd' })
        .collect();
    let deep = dir.dir(relative.trim_end_matches('/'));
    let path = deep.join("a.nu");
    fs::write(&path, "ls|first").unwrap();
    let good = dir.file("b.nu", "ls|first");
    let nuon = deep.join("nuten.nuon");
    let error = fs::symlink_metadata(&nuon).unwrap_err();
    // `a.nu` twice, so that a directory is looked up and reported once for both.
    let output = Command::new(env!("CARGO_BIN_EXE_nuten"))
        .args([
            OsStr::new("format"),
            OsStr::new("a.nu"),
            OsStr::new("a.nu"),
            good.as_os_str(),
        ])
        .current_dir(&deep)
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    assert_eq!(stderr(&output), io_error_line(&nuon, &error));
    assert_eq!(read(&path), "ls|first");
    assert_eq!(read(&good), "ls|first");
}

#[test]
fn format_reports_file_in_missing_directory_and_continues() {
    let dir = TempDir::new();
    let missing = dir.path("missing/a.nu");
    let good = dir.file("a.nu", "ls|first");
    let output = format(&[&missing, &good]);
    assert_eq!(output.status.code(), Some(2));
    let error = fs::read(&missing).unwrap_err();
    assert_eq!(stderr(&output), io_error_line(&missing, &error));
    assert_eq!(read(&good), "ls | first\n");
}

#[test]
fn format_reports_file_under_regular_file_and_continues() {
    let dir = TempDir::new();
    dir.file("a", "");
    let under = dir.path("a/a.nu");
    let good = dir.file("b.nu", "ls|first");
    let output = format(&[&under, &good]);
    assert_eq!(output.status.code(), Some(2));
    let error = fs::read(&under).unwrap_err();
    assert_eq!(stderr(&output), io_error_line(&under, &error));
    assert_eq!(read(&good), "ls | first\n");
}

#[cfg(unix)]
#[test]
fn format_reports_file_in_unsearchable_directory_alongside_wrong_config_file() {
    use std::os::unix::fs::PermissionsExt;

    let dir = TempDir::new();
    let locked = dir.file("a/a.nu", "ls|first");
    let json = dir.file("b/nuten.json", "{");
    let other = dir.file("b/a.nu", "ls|first");
    fs::set_permissions(dir.path("a"), fs::Permissions::from_mode(0o000)).unwrap();
    let error = fs::read(&locked).err();
    let output = format(&[&locked, &other]);
    fs::set_permissions(dir.path("a"), fs::Permissions::from_mode(0o755)).unwrap();
    // Root searches a directory without permission, so the test proves nothing then.
    let Some(error) = error else {
        return;
    };
    assert_eq!(output.status.code(), Some(2));
    assert_eq!(
        stderr(&output),
        io_error_line(&locked, &error) + &format!("{}:1:1: Unterminated object\n", json.display())
    );
    assert_eq!(read(&locked), "ls|first");
    assert_eq!(read(&other), "ls|first");
}

const NO_CONFIG_EXTENSION: &str =
    "config file needs one of the extensions `.nuon`, `.jsonc`, `.json`";

#[test]
fn format_takes_given_config_file_over_nearest() {
    let dir = TempDir::new();
    dir.file("nuten.nuon", "{indent_width: 3}");
    dir.file("style/two.json", r#"{"indent_width": 2}"#);
    let path = dir.file("a.nu", BLOCK);
    for flag in ["-c", "--config"] {
        fs::write(&path, BLOCK).unwrap();
        let output = format_in(&dir, &[flag, "style/two.json", "a.nu"]);
        assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
        assert_eq!(read(&path), "if true {\n  ls\n}\n", "{flag}");
    }
}

#[test]
fn format_takes_given_config_file_for_every_file() {
    let dir = TempDir::new();
    dir.file("a/nuten.nuon", "{indent_width: 3}");
    let config = dir.file("two.nuon", "{indent_width: 2}");
    let paths = ["a.nu", "a/a.nu", "b/a.nu"].map(|name| dir.file(name, BLOCK));
    let args: Vec<_> = [OsStr::new("format"), OsStr::new("-c"), config.as_os_str()]
        .into_iter()
        .chain(paths.iter().map(|path| path.as_os_str()))
        .collect();
    let output = nuten(&args);
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    for path in &paths {
        assert_eq!(read(path), "if true {\n  ls\n}\n", "{}", path.display());
    }
}

#[test]
fn format_leaves_wrong_nearest_config_file_unread_when_given_one() {
    let dir = TempDir::new();
    dir.file("nuten.nuon", "{");
    dir.file("two.nuon", "{indent_width: 2}");
    let path = dir.file("a.nu", BLOCK);
    let output = format_in(&dir, &["-c", "two.nuon", "a.nu"]);
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    assert_eq!(read(&path), "if true {\n  ls\n}\n");
}

#[test]
fn format_writes_nothing_when_given_config_file_cannot_be_read() {
    let dir = TempDir::new();
    dir.dir("dir.json");
    let path = dir.file("a.nu", "ls|first");
    let missing_error = fs::read(dir.path("missing.nuon")).unwrap_err();
    for (config, expected) in [
        (
            "missing.nuon",
            io_error_line(Path::new("missing.nuon"), &missing_error),
        ),
        ("dir.json", "dir.json: not a regular file\n".to_string()),
    ] {
        let output = format_in(&dir, &["-c", config, "a.nu"]);
        assert_eq!(output.status.code(), Some(2), "{config}");
        assert_eq!(stderr(&output), expected);
        assert_eq!(read(&path), "ls|first", "{config}");
    }
}

#[test]
fn format_rejects_given_config_file_without_config_extension() {
    let dir = TempDir::new();
    let path = dir.file("a.nu", "ls|first");
    for config in ["nuten.toml", "nuten", "nuten.JSON"] {
        dir.file(config, "{}");
        let output = format_in(&dir, &["-c", config, "a.nu"]);
        assert_eq!(output.status.code(), Some(2), "{config}");
        assert_eq!(
            stderr(&output),
            format!("{config}: {NO_CONFIG_EXTENSION}\n")
        );
        assert_eq!(read(&path), "ls|first", "{config}");
    }
}

#[test]
fn format_writes_nothing_when_given_config_file_is_wrong() {
    let dir = TempDir::new();
    dir.file("two.nuon", "{indent_width: 2, a: 1}");
    let path = dir.file("a.nu", "ls|first");
    let output = format_in(&dir, &["-c", "two.nuon", "a.nu"]);
    assert_eq!(output.status.code(), Some(2));
    assert_eq!(stderr(&output), "two.nuon:1:19: unknown key `a`\n");
    assert_eq!(read(&path), "ls|first");
}

#[test]
fn format_wants_value_for_config_flag() {
    let dir = TempDir::new();
    let path = dir.file("a.nu", "ls|first");
    let output = format_in(&dir, &["a.nu", "-c"]);
    assert_eq!(output.status.code(), Some(2));
    assert!(!output.stderr.is_empty());
    assert_eq!(read(&path), "ls|first");
}

#[test]
fn format_rejects_repeated_config_flag() {
    let dir = TempDir::new();
    dir.file("two.nuon", "{indent_width: 2}");
    dir.file("three.nuon", "{indent_width: 3}");
    let path = dir.file("a.nu", BLOCK);
    let output = format_in(&dir, &["-c", "two.nuon", "--config", "three.nuon", "a.nu"]);
    assert_eq!(output.status.code(), Some(2));
    let stderr = stderr(&output);
    assert_eq!(
        stderr.lines().next(),
        Some("error: the argument '--config' cannot be used multiple times"),
        "{stderr}"
    );
    assert_eq!(read(&path), BLOCK);
}

#[test]
fn format_takes_given_config_for_other_files_when_one_has_syntax_error() {
    let dir = TempDir::new();
    dir.file("two.nuon", "{indent_width: 2}");
    let bad = dir.file("bad.nu", "ls ||");
    let good = dir.file("good.nu", BLOCK);
    let output = format_in(&dir, &["-c", "two.nuon", "bad.nu", "good.nu"]);
    assert_eq!(output.status.code(), Some(2));
    assert_eq!(read(&bad), "ls ||");
    assert_eq!(read(&good), "if true {\n  ls\n}\n");
}

#[test]
fn wants_subcommand() {
    let output = nuten(&[]);
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
}

#[test]
fn rejects_file_without_subcommand() {
    let dir = TempDir::new();
    let path = dir.file("a.nu", "ls|first");
    let output = nuten(&[path.as_os_str()]);
    assert_eq!(output.status.code(), Some(2));
    assert_eq!(read(&path), "ls|first");
}

#[test]
fn schema_prints_json_schema_of_config_file() {
    let output = nuten(&[OsStr::new("schema")]);
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    assert!(output.stderr.is_empty());
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        nuten_config::schema()
    );
}

#[test]
fn schema_rejects_extra_argument() {
    let output = nuten(&[OsStr::new("schema"), OsStr::new("x")]);
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
}

#[test]
fn schema_succeeds_when_reader_closes_first() {
    let (reader, writer) = io::pipe().unwrap();
    drop(reader);
    let output = Command::new(env!("CARGO_BIN_EXE_nuten"))
        .arg("schema")
        .stdout(writer)
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    assert!(output.stderr.is_empty());
}

#[cfg(target_os = "linux")]
#[test]
fn schema_reports_write_error() {
    let full = fs::File::options().write(true).open("/dev/full").unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_nuten"))
        .arg("schema")
        .stdout(full)
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    assert!(!output.stderr.is_empty());
}
