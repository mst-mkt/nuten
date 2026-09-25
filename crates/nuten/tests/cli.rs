//! Runs the binary on files in a temporary directory, on stdin, and with its stdout closed or full.

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
/// No config file, ignore file, or repository of git is above it.
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
        let ignoring: Vec<_> = dir
            .0
            .ancestors()
            .flat_map(|ancestor| [".git", ".gitignore", ".ignore"].map(|name| ancestor.join(name)))
            .filter(|path| path.exists())
            .collect();
        assert!(
            ignoring.is_empty(),
            "{ignoring:?} above the temporary directory"
        );
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

/// The binary, blind to the git config of the user, which could ignore the files of a test.
fn nuten_command() -> Command {
    let nowhere = env::temp_dir().join("nuten-cli-nowhere");
    let mut command = Command::new(env!("CARGO_BIN_EXE_nuten"));
    command
        .env("HOME", &nowhere)
        .env("XDG_CONFIG_HOME", &nowhere)
        .env("GIT_CONFIG_SYSTEM", &nowhere)
        .env_remove("GIT_CONFIG_GLOBAL");
    command
}

fn nuten(args: &[&OsStr]) -> Output {
    nuten_command().args(args).output().unwrap()
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
    nuten_command()
        .arg("format")
        .args(args)
        .current_dir(&dir.0)
        .output()
        .unwrap()
}

/// Not a pipe, whose writer fails when the binary exits before it reads stdin.
fn format_stdin_command(dir: &TempDir, args: &[&str], input: &[u8]) -> Command {
    let input_path = dir.path("stdin.input");
    fs::write(&input_path, input).unwrap();
    let mut command = nuten_command();
    command
        .arg("format")
        .args(args)
        .current_dir(&dir.0)
        .stdin(fs::File::open(&input_path).unwrap());
    command
}

fn format_stdin_in(dir: &TempDir, args: &[&str], input: &[u8]) -> Output {
    format_stdin_command(dir, args, input).output().unwrap()
}

/// A block, which each indent width formats differently.
const BLOCK: &str = "if true {\nls\n}";

fn read(path: &Path) -> String {
    fs::read_to_string(path).unwrap()
}

fn stderr(output: &Output) -> String {
    String::from_utf8(output.stderr.clone()).unwrap()
}

fn reported_paths(output: &Output) -> Vec<String> {
    stderr(output)
        .lines()
        .map(|line| {
            let end = line.find(':').expect("a line reports a path");
            line[..end].to_string()
        })
        .collect()
}

fn stdout(output: &Output) -> String {
    String::from_utf8(output.stdout.clone()).unwrap()
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
fn format_rejects_unknown_flag() {
    let dir = TempDir::new();
    let [path] = write_unformatted(&dir, ["a.nu"]);
    let output = format_in(&dir, &["--bogus"]);
    assert_eq!(output.status.code(), Some(2));
    let stderr = stderr(&output);
    assert!(stderr.contains("--bogus"), "{stderr}");
    assert!(!stderr.contains("No such file"), "{stderr}");
    assert_eq!(read(&path), UNFORMATTED);
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
fn format_reports_files_in_order_given() {
    let dir = TempDir::new();
    // Many, so that they are spread over threads, and named against their order.
    let bad: Vec<_> = (0..64)
        .rev()
        .map(|index| dir.file(&format!("bad{index:02}.nu"), "ls ||"))
        .collect();
    let good: Vec<_> = (0..64)
        .map(|index| dir.file(&format!("good{index:02}.nu"), "ls|first"))
        .collect();
    let paths: Vec<_> = bad
        .iter()
        .zip(&good)
        .flat_map(|(bad, good)| [bad.as_path(), good.as_path()])
        .collect();
    let output = format(&paths);
    assert_eq!(output.status.code(), Some(2));
    let expected: Vec<_> = bad
        .iter()
        .map(|path| format!("{}:1:4", path.display()))
        .collect();
    let stderr = stderr(&output);
    let heads: Vec<_> = stderr
        .lines()
        .map(|line| &line[..line.find(": ").unwrap()])
        .collect();
    assert_eq!(heads, expected, "{stderr}");
    for path in &good {
        assert_eq!(read(path), "ls | first\n");
    }
}

#[test]
fn format_reports_file_given_by_several_paths_once() {
    let dir = TempDir::new();
    dir.dir("a");
    let bad = dir.file("bad.nu", "ls ||");
    let output = format_in(&dir, &["bad.nu", "a/../bad.nu", "bad.nu"]);
    assert_eq!(output.status.code(), Some(2));
    let stderr = stderr(&output);
    assert_eq!(stderr.lines().count(), 1, "{stderr}");
    assert!(stderr.starts_with("bad.nu:1:4: "), "{stderr}");
    assert_eq!(read(&bad), "ls ||");
}

#[test]
fn format_reports_missing_file_once_when_given_twice() {
    let dir = TempDir::new();
    let error = fs::read(dir.path("missing.nu")).unwrap_err();
    let output = format_in(&dir, &["missing.nu", "missing.nu"]);
    assert_eq!(output.status.code(), Some(2));
    assert_eq!(
        stderr(&output),
        io_error_line(Path::new("missing.nu"), &error)
    );
}

#[test]
fn format_reports_each_missing_file() {
    let dir = TempDir::new();
    let [a, b] = ["a.nu", "b.nu"].map(|name| fs::read(dir.path(name)).unwrap_err());
    let output = format_in(&dir, &["a.nu", "b.nu"]);
    assert_eq!(output.status.code(), Some(2));
    assert_eq!(
        stderr(&output),
        io_error_line(Path::new("a.nu"), &a) + &io_error_line(Path::new("b.nu"), &b)
    );
}

#[cfg(unix)]
#[test]
fn format_reports_file_given_with_symlink_to_it_once_at_first_path() {
    let dir = TempDir::new();
    let bad = dir.file("bad.nu", "ls ||");
    std::os::unix::fs::symlink(&bad, dir.path("link.nu")).unwrap();
    let output = format_in(&dir, &["link.nu", "bad.nu"]);
    assert_eq!(output.status.code(), Some(2));
    let stderr = stderr(&output);
    assert_eq!(stderr.lines().count(), 1, "{stderr}");
    assert!(stderr.starts_with("link.nu:1:4: "), "{stderr}");
}

#[cfg(unix)]
#[test]
fn format_formats_hard_linked_file_once_with_config_of_first_path() {
    let dir = TempDir::new();
    dir.file("a/nuten.nuon", "{indent_width: 2}");
    dir.file("b/nuten.nuon", "{indent_width: 3}");
    let path = dir.file("a/a.nu", BLOCK);
    fs::hard_link(&path, dir.path("b/a.nu")).unwrap();
    let output = format_in(&dir, &["a/a.nu", "b/a.nu"]);
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    assert_eq!(read(&path), "if true {\n  ls\n}\n");
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
fn format_takes_defaults_when_config_file_is_only_above_repository_root() {
    let dir = TempDir::new();
    dir.file("nuten.nuon", "{indent_width: 2}");
    dir.dir("a/.git");
    let path = dir.file("a/b/a.nu", BLOCK);
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

#[cfg(unix)]
fn fifo(path: &Path) {
    let status = Command::new("mkfifo").arg(path).status().unwrap();
    assert!(status.success());
}

/// Not `format`, which would hang along with the binary if it read a FIFO.
#[cfg(unix)]
fn format_in_time(paths: &[&Path]) -> Output {
    let mut child = nuten_command()
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
    let paths = ["a.nu", "b.nu"].map(|name| deep.join(name));
    for path in &paths {
        fs::write(path, "ls|first").unwrap();
    }
    let good = dir.file("b.nu", "ls|first");
    let nuon = deep.join("nuten.nuon");
    let error = fs::symlink_metadata(&nuon).unwrap_err();
    // Two files in one directory, so that it is looked up and reported once for both.
    let output = nuten_command()
        .args([
            OsStr::new("format"),
            OsStr::new("a.nu"),
            OsStr::new("b.nu"),
            good.as_os_str(),
        ])
        .current_dir(&deep)
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    assert_eq!(stderr(&output), io_error_line(&nuon, &error));
    for path in &paths {
        assert_eq!(read(path), "ls|first");
    }
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

const UNFORMATTED: &str = "ls|first";
const FORMATTED: &str = "ls | first\n";

fn write_unformatted<const N: usize>(dir: &TempDir, names: [&str; N]) -> [PathBuf; N] {
    names.map(|name| dir.file(name, UNFORMATTED))
}

fn assert_succeeds_quietly(output: &Output) {
    assert_eq!(output.status.code(), Some(0), "{}", stderr(output));
    assert!(output.stderr.is_empty(), "{}", stderr(output));
}

#[test]
fn format_formats_nu_files_under_directory() {
    let dir = TempDir::new();
    let nu = write_unformatted(&dir, ["d/a.nu", "d/e/f/b.nu"]);
    let other = write_unformatted(&dir, ["d/a.txt", "d/a.nu.bak", "d/nu", "d/e/NU.NU", "a.nu"]);
    let output = format_in(&dir, &["d"]);
    assert_succeeds_quietly(&output);
    for path in &nu {
        assert_eq!(read(path), FORMATTED, "{}", path.display());
    }
    for path in &other {
        assert_eq!(read(path), UNFORMATTED, "{}", path.display());
    }
}

#[test]
fn format_formats_hidden_files_under_directory() {
    let dir = TempDir::new();
    let paths = write_unformatted(&dir, ["d/.config/nushell/config.nu", "d/.a.nu"]);
    let output = format_in(&dir, &["d"]);
    assert_succeeds_quietly(&output);
    for path in &paths {
        assert_eq!(read(path), FORMATTED, "{}", path.display());
    }
}

#[test]
fn format_skips_version_control_directories_under_directory() {
    let dir = TempDir::new();
    let skipped = write_unformatted(
        &dir,
        [
            "d/.git/hooks/a.nu",
            "d/.hg/a.nu",
            "d/.svn/a.nu",
            "d/.bzr/a.nu",
            "d/.jj/a.nu",
            "d/e/.git/a.nu",
        ],
    );
    let taken = write_unformatted(&dir, ["d/.github/a.nu", "d/git/a.nu"]);
    let output = format_in(&dir, &["d"]);
    assert_succeeds_quietly(&output);
    for path in &skipped {
        assert_eq!(read(path), UNFORMATTED, "{}", path.display());
    }
    for path in &taken {
        assert_eq!(read(path), FORMATTED, "{}", path.display());
    }
}

#[test]
fn format_skips_files_gitignore_ignores_in_git_repository() {
    let dir = TempDir::new();
    dir.dir("d/.git");
    dir.file("d/.gitignore", "gen/\n*.gen.nu\n");
    dir.file("d/e/.gitignore", "local.nu\n");
    let ignored = write_unformatted(&dir, ["d/gen/a.nu", "d/a.gen.nu", "d/e/local.nu"]);
    let taken = write_unformatted(&dir, ["d/a.nu", "d/local.nu"]);
    let output = format_in(&dir, &["d"]);
    assert_succeeds_quietly(&output);
    for path in &ignored {
        assert_eq!(read(path), UNFORMATTED, "{}", path.display());
    }
    for path in &taken {
        assert_eq!(read(path), FORMATTED, "{}", path.display());
    }
}

#[test]
fn format_skips_files_gitignore_above_directory_ignores() {
    let dir = TempDir::new();
    dir.dir(".git");
    dir.file(".gitignore", "gen/\n");
    let [ignored, taken] = write_unformatted(&dir, ["d/gen/a.nu", "d/a.nu"]);
    let output = format_in(&dir, &["d"]);
    assert_succeeds_quietly(&output);
    assert_eq!(read(&ignored), UNFORMATTED);
    assert_eq!(read(&taken), FORMATTED);
}

#[test]
fn format_takes_no_gitignore_above_git_repository() {
    let dir = TempDir::new();
    dir.file(".gitignore", "gen/\n");
    dir.dir("d/.git");
    let [path] = write_unformatted(&dir, ["d/gen/a.nu"]);
    let output = format_in(&dir, &["d"]);
    assert_succeeds_quietly(&output);
    assert_eq!(read(&path), FORMATTED);
}

#[test]
fn format_skips_files_gitignore_ignores_when_dot_git_is_file() {
    let dir = TempDir::new();
    dir.file("d/.git", "gitdir: ../elsewhere\n");
    dir.file("d/.gitignore", "gen/\n");
    let [ignored, taken] = write_unformatted(&dir, ["d/gen/a.nu", "d/a.nu"]);
    let output = format_in(&dir, &["d"]);
    assert_succeeds_quietly(&output);
    assert_eq!(read(&ignored), UNFORMATTED);
    assert_eq!(read(&taken), FORMATTED);
}

#[test]
fn format_skips_files_git_exclude_ignores() {
    let dir = TempDir::new();
    dir.file("d/.git/info/exclude", "gen/\n");
    let [ignored, taken] = write_unformatted(&dir, ["d/gen/a.nu", "d/a.nu"]);
    let output = format_in(&dir, &["d"]);
    assert_succeeds_quietly(&output);
    assert_eq!(read(&ignored), UNFORMATTED);
    assert_eq!(read(&taken), FORMATTED);
}

#[test]
fn format_skips_files_global_excludes_file_of_git_ignores() {
    let dir = TempDir::new();
    dir.file("config/git/ignore", "gen/\n");
    let config_home = dir.path("config");
    dir.dir("d/.git");
    let [ignored, taken] = write_unformatted(&dir, ["d/gen/a.nu", "d/a.nu"]);
    let output = nuten_command()
        .args(["format", "d"])
        .env("XDG_CONFIG_HOME", config_home)
        .current_dir(&dir.0)
        .output()
        .unwrap();
    assert_succeeds_quietly(&output);
    assert_eq!(read(&ignored), UNFORMATTED);
    assert_eq!(read(&taken), FORMATTED);
}

#[test]
fn format_takes_no_gitignore_outside_git_repository() {
    let dir = TempDir::new();
    dir.file("d/.gitignore", "a.nu\n");
    let [path] = write_unformatted(&dir, ["d/a.nu"]);
    let output = format_in(&dir, &["d"]);
    assert_succeeds_quietly(&output);
    assert_eq!(read(&path), FORMATTED);
}

#[test]
fn format_skips_files_dot_ignore_ignores_outside_git_repository() {
    let dir = TempDir::new();
    dir.file("d/.ignore", "a.nu\n");
    let [ignored, taken] = write_unformatted(&dir, ["d/a.nu", "d/b.nu"]);
    let output = format_in(&dir, &["d"]);
    assert_succeeds_quietly(&output);
    assert_eq!(read(&ignored), UNFORMATTED);
    assert_eq!(read(&taken), FORMATTED);
}

#[test]
fn format_formats_ignored_file_given_directly() {
    let dir = TempDir::new();
    dir.dir(".git");
    dir.file(".gitignore", "a.nu\n");
    let [path] = write_unformatted(&dir, ["a.nu"]);
    let output = format_in(&dir, &["a.nu"]);
    assert_succeeds_quietly(&output);
    assert_eq!(read(&path), FORMATTED);
}

#[test]
fn format_walks_directory_given_directly_though_ignored_or_version_control_directory() {
    let dir = TempDir::new();
    dir.dir(".git");
    dir.file(".gitignore", "gen/\n");
    let paths = write_unformatted(&dir, ["gen/a.nu", ".git/a.nu"]);
    let output = format_in(&dir, &["gen", ".git"]);
    assert_succeeds_quietly(&output);
    for path in &paths {
        assert_eq!(read(path), FORMATTED, "{}", path.display());
    }
}

#[test]
fn format_reports_ignore_file_line_it_cannot_read_and_leaves_files_under_it_alone() {
    let dir = TempDir::new();
    let gitignore = dir.file("d/e/.gitignore", "gen/\na{b\n");
    let skipped = write_unformatted(&dir, ["d/e/a.nu", "d/e/f/a.nu"]);
    let [taken] = write_unformatted(&dir, ["d/a.nu"]);
    let output = format_in(&dir, &["d"]);
    assert_eq!(output.status.code(), Some(2));
    let stderr = stderr(&output);
    assert_eq!(stderr.lines().count(), 1, "{stderr}");
    assert!(
        stderr.starts_with(&format!("{}:2: ", gitignore.display())),
        "{stderr}"
    );
    for path in &skipped {
        assert_eq!(read(path), UNFORMATTED, "{}", path.display());
    }
    assert_eq!(read(&taken), FORMATTED);
}

#[test]
fn format_leaves_directory_given_alone_when_its_ignore_file_has_line_it_cannot_read() {
    let dir = TempDir::new();
    let ignore = dir.file("d/.ignore", "a{b\n");
    let [skipped, taken] = write_unformatted(&dir, ["d/a.nu", "b.nu"]);
    let output = format_in(&dir, &["d", "b.nu"]);
    assert_eq!(output.status.code(), Some(2));
    let stderr = stderr(&output);
    assert_eq!(stderr.lines().count(), 1, "{stderr}");
    assert!(
        stderr.starts_with(&format!("{}:1: ", ignore.display())),
        "{stderr}"
    );
    assert_eq!(read(&skipped), UNFORMATTED);
    assert_eq!(read(&taken), FORMATTED);
}

#[test]
fn format_reports_each_ignore_file_line_it_cannot_read() {
    let dir = TempDir::new();
    let ignore = dir.file("d/.ignore", "a{b\nc\nd{e\n");
    let output = format_in(&dir, &["d"]);
    assert_eq!(output.status.code(), Some(2));
    let stderr = stderr(&output);
    let lines: Vec<_> = stderr.lines().collect();
    assert_eq!(lines.len(), 2, "{stderr}");
    assert!(
        lines[0].starts_with(&format!("{}:1: ", ignore.display())),
        "{stderr}"
    );
    assert!(
        lines[1].starts_with(&format!("{}:3: ", ignore.display())),
        "{stderr}"
    );
}

#[test]
fn format_leaves_directory_alone_when_ignore_file_above_it_has_line_it_cannot_read() {
    let dir = TempDir::new();
    let ignore = dir.file("d/.ignore", "a{b\n");
    let [skipped] = write_unformatted(&dir, ["d/e/a.nu"]);
    let output = format_in(&dir, &["d/e"]);
    assert_eq!(output.status.code(), Some(2));
    let stderr = stderr(&output);
    assert_eq!(stderr.lines().count(), 1, "{stderr}");
    assert!(
        stderr.starts_with(&format!("{}:1: ", ignore.display())),
        "{stderr}"
    );
    assert_eq!(read(&skipped), UNFORMATTED);
}

#[test]
fn format_leaves_directories_alone_when_global_excludes_file_of_git_has_line_it_cannot_read() {
    let dir = TempDir::new();
    let excludes = dir.file("config/git/ignore", "a{b\n");
    let config_home = dir.path("config");
    let [skipped, taken] = write_unformatted(&dir, ["d/a.nu", "b.nu"]);
    let output = nuten_command()
        .args(["format", "d", "b.nu"])
        .env("XDG_CONFIG_HOME", config_home)
        .current_dir(&dir.0)
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    let stderr = stderr(&output);
    assert_eq!(stderr.lines().count(), 1, "{stderr}");
    assert!(
        stderr.starts_with(&format!("{}:1: ", excludes.display())),
        "{stderr}"
    );
    assert_eq!(read(&skipped), UNFORMATTED);
    assert_eq!(read(&taken), FORMATTED);
}

#[test]
fn format_reports_problem_of_directory_once_when_given_twice() {
    let dir = TempDir::new();
    let ignore = dir.file("d/.ignore", "a{b\n");
    let output = format_in(&dir, &["d", "d"]);
    assert_eq!(output.status.code(), Some(2));
    let stderr = stderr(&output);
    assert_eq!(stderr.lines().count(), 1, "{stderr}");
    assert!(
        stderr.starts_with(&format!("{}:1: ", ignore.display())),
        "{stderr}"
    );
}

#[test]
fn format_reports_problem_of_ignore_file_once_when_above_one_directory_and_in_another() {
    let dir = TempDir::new();
    dir.file("d/.ignore", "a{b\n");
    dir.dir("d/e");
    for paths in [["d", "d/e"], ["d/e", "d"]] {
        let output = format_in(&dir, &paths);
        assert_eq!(output.status.code(), Some(2));
        let stderr = stderr(&output);
        assert_eq!(stderr.lines().count(), 1, "{paths:?}: {stderr}");
    }
}

#[cfg(unix)]
#[test]
fn format_reports_unreadable_current_directory_by_dot() {
    use std::os::unix::fs::PermissionsExt;

    let dir = TempDir::new();
    fs::set_permissions(&dir.0, fs::Permissions::from_mode(0o300)).unwrap();
    let error = fs::read_dir(&dir.0).err();
    let outputs = [&["."][..], &["./"], &[".", "./"]].map(|paths| format_in(&dir, paths));
    fs::set_permissions(&dir.0, fs::Permissions::from_mode(0o755)).unwrap();
    // Root reads a directory without permission, so the test proves nothing then.
    let Some(error) = error else {
        return;
    };
    for output in &outputs {
        assert_eq!(output.status.code(), Some(2));
        assert_eq!(stderr(output), io_error_line(Path::new("."), &error));
    }
}

#[cfg(unix)]
#[test]
fn format_reports_unreadable_directory_once_when_given_with_dot_or_trailing_slash() {
    use std::os::unix::fs::PermissionsExt;

    let dir = TempDir::new();
    let unreadable = dir.dir("u");
    fs::set_permissions(&unreadable, fs::Permissions::from_mode(0o300)).unwrap();
    let error = fs::read_dir(&unreadable).err();
    let output = format_in(&dir, &["u", "u/", "u/.", "./u/"]);
    fs::set_permissions(&unreadable, fs::Permissions::from_mode(0o755)).unwrap();
    // Root reads a directory without permission, so the test proves nothing then.
    let Some(error) = error else {
        return;
    };
    assert_eq!(output.status.code(), Some(2));
    assert_eq!(stderr(&output), io_error_line(Path::new("u"), &error));
}

#[cfg(unix)]
#[test]
fn format_walks_no_symlink_under_directory() {
    let dir = TempDir::new();
    let outside = write_unformatted(&dir, ["out/a.nu", "out/e/b.nu"]);
    dir.dir("d");
    std::os::unix::fs::symlink(&outside[0], dir.path("d/link.nu")).unwrap();
    std::os::unix::fs::symlink(dir.path("out/e"), dir.path("d/link")).unwrap();
    let output = format_in(&dir, &["d"]);
    assert_succeeds_quietly(&output);
    for path in &outside {
        assert_eq!(read(path), UNFORMATTED, "{}", path.display());
    }
}

#[cfg(unix)]
#[test]
fn format_walks_symlink_to_directory_given_directly() {
    let dir = TempDir::new();
    let [path] = write_unformatted(&dir, ["real/a.nu"]);
    std::os::unix::fs::symlink(dir.path("real"), dir.path("link")).unwrap();
    let output = format_in(&dir, &["link"]);
    assert_succeeds_quietly(&output);
    assert_eq!(read(&path), FORMATTED);
}

#[cfg(unix)]
#[test]
fn format_skips_fifo_under_directory_without_waiting() {
    let dir = TempDir::new();
    fifo(&dir.dir("d").join("fifo.nu"));
    let [path] = write_unformatted(&dir, ["d/a.nu"]);
    let output = format_in_time(&[&dir.path("d")]);
    assert_succeeds_quietly(&output);
    assert_eq!(read(&path), FORMATTED);
}

#[test]
fn format_succeeds_on_directory_without_nu_files() {
    let dir = TempDir::new();
    dir.file("d/a.txt", UNFORMATTED);
    dir.dir("e");
    let output = format_in(&dir, &["d", "e"]);
    assert_succeeds_quietly(&output);
}

#[cfg(unix)]
#[test]
fn format_reports_files_under_directory_in_path_order_where_directory_is_given() {
    let dir = TempDir::new();
    for name in ["z.nu", "d/b.nu", "d/a.nu", "d/a/c.nu", "y.nu"] {
        dir.file(name, "ls ||");
    }
    let output = format_in(&dir, &["z.nu", "d", "y.nu"]);
    assert_eq!(output.status.code(), Some(2));
    let stderr = stderr(&output);
    let heads: Vec<_> = stderr
        .lines()
        .map(|line| &line[..line.find(':').unwrap()])
        .collect();
    assert_eq!(
        heads,
        ["z.nu", "d/a/c.nu", "d/a.nu", "d/b.nu", "y.nu"],
        "{stderr}"
    );
}

#[cfg(unix)]
#[test]
fn format_reports_files_under_current_directory_by_paths_relative_to_it() {
    let dir = TempDir::new();
    dir.file("a/bad.nu", "ls ||");
    for given in [".", "./"] {
        let output = format_in(&dir, &[given]);
        assert_eq!(output.status.code(), Some(2));
        let stderr = stderr(&output);
        assert!(stderr.starts_with("a/bad.nu:1:4: "), "{given}: {stderr}");
    }
}

#[cfg(unix)]
#[test]
fn format_reports_file_under_overlapping_directories_once_at_first_path() {
    let dir = TempDir::new();
    dir.file("d/e/bad.nu", "ls ||");
    let output = format_in(&dir, &["d/e", "d", "d/e/bad.nu"]);
    assert_eq!(output.status.code(), Some(2));
    let stderr = stderr(&output);
    assert_eq!(stderr.lines().count(), 1, "{stderr}");
    assert!(stderr.starts_with("d/e/bad.nu:1:4: "), "{stderr}");
}

#[test]
fn format_takes_nearest_config_file_for_each_file_under_directory() {
    let dir = TempDir::new();
    dir.file("d/nuten.nuon", "{indent_width: 2}");
    dir.file("d/e/nuten.nuon", "{indent_width: 3}");
    let top = dir.file("d/a.nu", BLOCK);
    let inner = dir.file("d/e/a.nu", BLOCK);
    let output = format_in(&dir, &["d"]);
    assert_succeeds_quietly(&output);
    assert_eq!(read(&top), "if true {\n  ls\n}\n");
    assert_eq!(read(&inner), "if true {\n   ls\n}\n");
}

#[test]
fn format_writes_nothing_when_config_file_under_directory_is_wrong() {
    let dir = TempDir::new();
    let nuon = dir.file("d/e/nuten.nuon", "{a: 1}");
    let paths = write_unformatted(&dir, ["d/a.nu", "d/e/a.nu"]);
    let output = format_in(&dir, &["d"]);
    assert_eq!(output.status.code(), Some(2));
    assert_eq!(
        stderr(&output),
        format!("{}:1:2: unknown key `a`\n", nuon.display())
    );
    for path in &paths {
        assert_eq!(read(path), UNFORMATTED, "{}", path.display());
    }
}

#[cfg(unix)]
#[test]
fn format_reports_unreadable_directory_under_directory_and_continues() {
    use std::os::unix::fs::PermissionsExt;

    let dir = TempDir::new();
    let [locked, taken] = write_unformatted(&dir, ["d/locked/a.nu", "d/b.nu"]);
    let locked_dir = dir.path("d/locked");
    fs::set_permissions(&locked_dir, fs::Permissions::from_mode(0o000)).unwrap();
    let error = fs::read_dir(&locked_dir).err();
    let output = format_in(&dir, &["d"]);
    fs::set_permissions(&locked_dir, fs::Permissions::from_mode(0o755)).unwrap();
    // Root reads a directory without permission, so the test proves nothing then.
    let Some(error) = error else {
        return;
    };
    assert_eq!(output.status.code(), Some(2));
    assert_eq!(
        stderr(&output),
        io_error_line(Path::new("d/locked"), &error)
    );
    assert_eq!(read(&locked), UNFORMATTED);
    assert_eq!(read(&taken), FORMATTED);
}

#[cfg(unix)]
#[test]
fn format_reports_problems_of_walking_and_of_formatting_in_order_of_paths_given() {
    let dir = TempDir::new();
    dir.file("z.nu", "ls ||");
    dir.file("y.nu", "ls ||");
    let ignore = dir.file("d/.ignore", "a{b\n").display().to_string();
    for (paths, expected) in [
        (["z.nu", "d", "y.nu"], ["z.nu", &ignore, "y.nu"]),
        (["y.nu", "z.nu", "d"], ["y.nu", "z.nu", &ignore]),
    ] {
        let output = format_in(&dir, &paths);
        assert_eq!(output.status.code(), Some(2));
        assert_eq!(reported_paths(&output), expected, "{}", stderr(&output));
    }
}

#[cfg(unix)]
#[test]
fn format_reports_problems_met_while_walking_directory_among_its_files() {
    let dir = TempDir::new();
    dir.file("d/a.nu", "ls ||");
    let ignore = dir.file("d/b/.ignore", "a{b\n").display().to_string();
    dir.file("d/b/x.nu", "ls ||");
    dir.file("d/c.nu", "ls ||");
    let output = format_in(&dir, &["d"]);
    assert_eq!(output.status.code(), Some(2));
    assert_eq!(
        reported_paths(&output),
        ["d/a.nu", &ignore, "d/c.nu"],
        "{}",
        stderr(&output)
    );
}

#[cfg(unix)]
#[test]
fn format_reports_path_it_cannot_resolve_where_it_is_given() {
    let dir = TempDir::new();
    dir.file("z.nu", "ls ||");
    dir.file("y.nu", "ls ||");
    let output = format_in(&dir, &["z.nu", "missing/a.nu", "y.nu"]);
    assert_eq!(output.status.code(), Some(2));
    assert_eq!(
        reported_paths(&output),
        ["z.nu", "missing/a.nu", "y.nu"],
        "{}",
        stderr(&output)
    );
}

#[cfg(unix)]
#[test]
fn format_reports_wrong_config_file_once_where_first_path_it_configures_is_given() {
    let dir = TempDir::new();
    let ignore = dir.file("d/.ignore", "a{b\n").display().to_string();
    let nuon = dir.file("e/nuten.nuon", "{a: 1}").display().to_string();
    let paths = write_unformatted(&dir, ["e/a.nu", "e/b.nu"]);
    for (args, expected) in [
        (
            &["d", "e/a.nu", "e/b.nu"][..],
            &[ignore.as_str(), &nuon][..],
        ),
        (&["e/b.nu", "d", "e/a.nu"], &[&nuon, &ignore]),
        (&["e", "d"], &[&nuon, &ignore]),
        (&["d", "e"], &[&ignore, &nuon]),
        (&["e/a.nu", "e"], &[&nuon]),
    ] {
        let output = format_in(&dir, args);
        assert_eq!(output.status.code(), Some(2));
        assert_eq!(reported_paths(&output), expected, "{}", stderr(&output));
        for path in &paths {
            assert_eq!(read(path), UNFORMATTED, "{}", path.display());
        }
    }
}

#[cfg(unix)]
#[test]
fn format_reports_problem_of_global_excludes_file_once_at_first_directory_respecting_it() {
    let dir = TempDir::new();
    let excludes = dir.file("config/git/ignore", "a{b\n").display().to_string();
    dir.file("z.nu", "ls ||");
    dir.file("off/nuten.nuon", "{respect_gitignore: false}");
    dir.file("off/y.nu", "ls ||");
    let skipped = write_unformatted(&dir, ["on1/a.nu", "on2/a.nu"]);
    let output = nuten_command()
        .args(["format", "z.nu", "off", "on1", "on2"])
        .env("XDG_CONFIG_HOME", dir.path("config"))
        .current_dir(&dir.0)
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    assert_eq!(
        reported_paths(&output),
        ["z.nu", "off/y.nu", &excludes],
        "{}",
        stderr(&output)
    );
    for path in &skipped {
        assert_eq!(read(path), UNFORMATTED, "{}", path.display());
    }
}

#[test]
fn format_formats_ignored_files_when_respect_gitignore_is_off() {
    let dir = TempDir::new();
    dir.file("d/nuten.nuon", "{respect_gitignore: false}");
    dir.dir("d/.git");
    dir.file("d/.gitignore", "gen/\n");
    dir.file("d/.ignore", "local.nu\n");
    let taken = write_unformatted(&dir, ["d/gen/a.nu", "d/local.nu"]);
    let [vcs] = write_unformatted(&dir, ["d/.git/a.nu"]);
    let output = format_in(&dir, &["d"]);
    assert_succeeds_quietly(&output);
    for path in &taken {
        assert_eq!(read(path), FORMATTED, "{}", path.display());
    }
    assert_eq!(read(&vcs), UNFORMATTED);
}

#[test]
fn format_reads_no_ignore_file_when_respect_gitignore_is_off() {
    let dir = TempDir::new();
    dir.file("d/nuten.nuon", "{respect_gitignore: false}");
    dir.file("d/.ignore", "a{b\n");
    dir.file("config/git/ignore", "a{b\n");
    let [path] = write_unformatted(&dir, ["d/a.nu"]);
    let output = nuten_command()
        .args(["format", "d"])
        .env("XDG_CONFIG_HOME", dir.path("config"))
        .current_dir(&dir.0)
        .output()
        .unwrap();
    assert_succeeds_quietly(&output);
    assert_eq!(read(&path), FORMATTED);
}

#[test]
fn format_takes_respect_gitignore_from_config_file_nearest_directory_given() {
    let dir = TempDir::new();
    dir.file("d/nuten.nuon", "{respect_gitignore: false}");
    dir.file("d/e/nuten.nuon", "{respect_gitignore: true}");
    dir.dir("d/.git");
    dir.file("d/.gitignore", "gen/\n");
    let [path] = write_unformatted(&dir, ["d/e/gen/a.nu"]);
    let output = format_in(&dir, &["d/e"]);
    assert_succeeds_quietly(&output);
    assert_eq!(read(&path), UNFORMATTED);
    let output = format_in(&dir, &["d"]);
    assert_succeeds_quietly(&output);
    assert_eq!(read(&path), FORMATTED);
}

#[cfg(unix)]
#[test]
fn format_takes_respect_gitignore_from_config_file_where_symlinked_directory_leads() {
    let dir = TempDir::new();
    dir.file("real/nuten.nuon", "{respect_gitignore: false}");
    dir.file("real/sub/.ignore", "a.nu\n");
    let [path] = write_unformatted(&dir, ["real/sub/a.nu"]);
    dir.file("e/nuten.nuon", "{respect_gitignore: true}");
    std::os::unix::fs::symlink(dir.path("real/sub"), dir.path("e/link")).unwrap();
    let output = format_in(&dir, &["e/link"]);
    assert_succeeds_quietly(&output);
    assert_eq!(read(&path), FORMATTED);
}

#[test]
fn format_reads_no_global_excludes_file_of_git_when_respect_gitignore_is_off() {
    let dir = TempDir::new();
    dir.file("config/git/ignore", "a{b\n");
    dir.file("off/nuten.nuon", "{respect_gitignore: false}");
    let [on, off] = write_unformatted(&dir, ["on/a.nu", "off/a.nu"]);
    let output = nuten_command()
        .args(["format", "on", "off"])
        .env("XDG_CONFIG_HOME", dir.path("config"))
        .current_dir(&dir.0)
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    let stderr = stderr(&output);
    assert_eq!(stderr.lines().count(), 1, "{stderr}");
    assert_eq!(read(&on), UNFORMATTED);
    assert_eq!(read(&off), FORMATTED);
}

#[test]
fn format_takes_respect_gitignore_from_given_config_file() {
    let dir = TempDir::new();
    dir.file("d/nuten.nuon", "{respect_gitignore: true}");
    dir.file("off.nuon", "{respect_gitignore: false}");
    dir.file("d/.ignore", "a.nu\n");
    let [path] = write_unformatted(&dir, ["d/a.nu"]);
    let output = format_in(&dir, &["-c", "off.nuon", "d"]);
    assert_succeeds_quietly(&output);
    assert_eq!(read(&path), FORMATTED);
}

#[test]
fn format_reads_no_config_file_of_directory_when_given_config_file() {
    let dir = TempDir::new();
    dir.file("d/nuten.nuon", "{");
    dir.file("two.nuon", "{indent_width: 2}");
    let path = dir.file("d/a.nu", BLOCK);
    let output = format_in(&dir, &["-c", "two.nuon", "d"]);
    assert_succeeds_quietly(&output);
    assert_eq!(read(&path), "if true {\n  ls\n}\n");
}

#[test]
fn format_reports_missing_file_once_when_given_config_file() {
    let dir = TempDir::new();
    dir.file("two.nuon", "{indent_width: 2}");
    let error = fs::read(dir.path("missing.nu")).unwrap_err();
    let output = format_in(&dir, &["-c", "two.nuon", "missing.nu"]);
    assert_eq!(output.status.code(), Some(2));
    assert_eq!(
        stderr(&output),
        io_error_line(Path::new("missing.nu"), &error)
    );
}

#[test]
fn format_writes_nothing_when_config_file_of_directory_is_wrong() {
    let dir = TempDir::new();
    let nuon = dir.file("d/nuten.nuon", "{respect_gitignore: 1}");
    let paths = write_unformatted(&dir, ["d/a.nu", "a.nu"]);
    let output = format_in(&dir, &["a.nu", "d"]);
    assert_eq!(output.status.code(), Some(2));
    assert_eq!(
        stderr(&output),
        format!(
            "{}:1:21: `respect_gitignore` takes a boolean, not an integer\n",
            nuon.display()
        )
    );
    for path in &paths {
        assert_eq!(read(path), UNFORMATTED, "{}", path.display());
    }
}

#[test]
fn format_formats_current_directory_without_paths() {
    let dir = TempDir::new();
    dir.dir(".git");
    dir.file(".gitignore", "gen/\n");
    let [taken, ignored] = write_unformatted(&dir, ["a/a.nu", "gen/a.nu"]);
    let output = format_in(&dir, &[]);
    assert_succeeds_quietly(&output);
    assert_eq!(read(&taken), FORMATTED);
    assert_eq!(read(&ignored), UNFORMATTED);
}

#[cfg(unix)]
#[test]
fn format_reports_files_under_current_directory_without_paths_by_paths_relative_to_it() {
    let dir = TempDir::new();
    dir.file("a/bad.nu", "ls ||");
    let output = format_in(&dir, &[]);
    assert_eq!(output.status.code(), Some(2));
    let stderr = stderr(&output);
    assert!(stderr.starts_with("a/bad.nu:1:4: "), "{stderr}");
}

#[test]
fn format_takes_respect_gitignore_from_config_file_above_current_directory_without_paths() {
    let dir = TempDir::new();
    dir.file("c/nuten.nuon", "{respect_gitignore: false}");
    dir.file("c/d/.ignore", "a.nu\n");
    let [path] = write_unformatted(&dir, ["c/d/a.nu"]);
    let output = nuten_command()
        .arg("format")
        .current_dir(dir.path("c/d"))
        .output()
        .unwrap();
    assert_succeeds_quietly(&output);
    assert_eq!(read(&path), FORMATTED);
}

#[test]
fn format_takes_given_config_file_for_current_directory_without_paths() {
    let dir = TempDir::new();
    dir.file("two.nuon", "{indent_width: 2}");
    let path = dir.file("a.nu", BLOCK);
    let output = format_in(&dir, &["-c", "two.nuon"]);
    assert_succeeds_quietly(&output);
    assert_eq!(read(&path), "if true {\n  ls\n}\n");
}

#[test]
fn format_check_lists_files_it_would_change_in_order_given_and_changes_none() {
    let dir = TempDir::new();
    let [z, a] = write_unformatted(&dir, ["z.nu", "d/a.nu"]);
    let formatted = dir.file("b.nu", FORMATTED);
    let output = format_in(&dir, &["--check", "z.nu", "b.nu", "d"]);
    assert_eq!(output.status.code(), Some(1), "{}", stderr(&output));
    assert!(output.stderr.is_empty(), "{}", stderr(&output));
    assert_eq!(stdout(&output), "z.nu\nd/a.nu\n");
    assert_eq!(read(&z), UNFORMATTED);
    assert_eq!(read(&a), UNFORMATTED);
    assert_eq!(read(&formatted), FORMATTED);
}

#[test]
fn format_check_succeeds_quietly_when_every_file_is_formatted() {
    let dir = TempDir::new();
    dir.file("a.nu", FORMATTED);
    dir.file("nuten.nuon", "{indent_width: 2}");
    dir.file("b.nu", "if true {\n  ls\n}\n");
    let output = format_in(&dir, &["--check"]);
    assert_succeeds_quietly(&output);
    assert!(output.stdout.is_empty());
}

#[test]
fn format_check_exits_with_2_on_problem_though_files_would_change() {
    let dir = TempDir::new();
    let bad = dir.file("bad.nu", "ls ||");
    let [path] = write_unformatted(&dir, ["a.nu"]);
    let output = format_in(&dir, &["--check", "bad.nu", "a.nu"]);
    assert_eq!(output.status.code(), Some(2));
    assert_eq!(stdout(&output), "a.nu\n");
    assert!(
        stderr(&output).starts_with("bad.nu:1:4: "),
        "{}",
        stderr(&output)
    );
    assert_eq!(read(&bad), "ls ||");
    assert_eq!(read(&path), UNFORMATTED);
}

#[test]
fn format_check_lists_nothing_when_any_config_file_is_wrong() {
    let dir = TempDir::new();
    dir.file("bad/nuten.nuon", "{a: 1}");
    let paths = write_unformatted(&dir, ["bad/a.nu", "ok/b.nu"]);
    for args in [["--check", "bad", "ok"], ["--check", "ok/b.nu", "bad"]] {
        let output = format_in(&dir, &args);
        assert_eq!(output.status.code(), Some(2), "{args:?}");
        assert!(output.stdout.is_empty(), "{args:?}: {}", stdout(&output));
    }
    for path in &paths {
        assert_eq!(read(path), UNFORMATTED, "{}", path.display());
    }
}

#[test]
fn format_check_counts_list_as_written_when_reader_closes_first() {
    let dir = TempDir::new();
    write_unformatted(&dir, ["a.nu"]);
    let (reader, writer) = io::pipe().unwrap();
    drop(reader);
    let output = nuten_command()
        .args(["format", "--check", "a.nu"])
        .current_dir(&dir.0)
        .stdout(writer)
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1), "{}", stderr(&output));
    assert!(output.stderr.is_empty());
}

#[cfg(target_os = "linux")]
#[test]
fn format_check_reports_list_it_cannot_write() {
    let dir = TempDir::new();
    let [path] = write_unformatted(&dir, ["a.nu"]);
    let full = fs::File::options().write(true).open("/dev/full").unwrap();
    let output = nuten_command()
        .args(["format", "--check", "a.nu"])
        .current_dir(&dir.0)
        .stdout(full)
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    let stderr = stderr(&output);
    assert!(stderr.starts_with("stdout: "), "{stderr}");
    assert_eq!(read(&path), UNFORMATTED);
}

#[test]
fn format_writes_stdin_formatted_to_stdout() {
    let dir = TempDir::new();
    let output = format_stdin_in(&dir, &["-"], UNFORMATTED.as_bytes());
    assert_succeeds_quietly(&output);
    assert_eq!(stdout(&output), FORMATTED);
}

#[test]
fn format_takes_config_file_nearest_current_directory_for_stdin() {
    let dir = TempDir::new();
    dir.file("nuten.nuon", "{indent_width: 2}");
    let output = format_stdin_in(&dir, &["-"], BLOCK.as_bytes());
    assert_succeeds_quietly(&output);
    assert_eq!(stdout(&output), "if true {\n  ls\n}\n");
}

#[test]
fn format_takes_given_config_file_for_stdin() {
    let dir = TempDir::new();
    dir.file("nuten.nuon", "{indent_width: 3}");
    dir.file("two.nuon", "{indent_width: 2}");
    let output = format_stdin_in(&dir, &["-c", "two.nuon", "-"], BLOCK.as_bytes());
    assert_succeeds_quietly(&output);
    assert_eq!(stdout(&output), "if true {\n  ls\n}\n");
}

#[test]
fn format_writes_nothing_to_stdout_and_reports_syntax_error_of_stdin() {
    let dir = TempDir::new();
    let output = format_stdin_in(&dir, &["-"], b"ls\nls ||");
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    let stderr = stderr(&output);
    assert_eq!(stderr.lines().count(), 1, "{stderr}");
    assert!(stderr.starts_with("<stdin>:2:4: "), "{stderr}");
}

#[test]
fn format_reports_stdin_that_is_not_utf8() {
    let dir = TempDir::new();
    let output = format_stdin_in(&dir, &["-"], b"ls \xff");
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    let stderr = stderr(&output);
    assert!(stderr.starts_with("<stdin>: "), "{stderr}");
}

#[test]
fn format_writes_nothing_to_stdout_when_config_file_for_stdin_is_wrong() {
    let dir = TempDir::new();
    let nuon = dir.file("nuten.nuon", "{a: 1}");
    let output = format_stdin_in(&dir, &["-"], UNFORMATTED.as_bytes());
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    assert_eq!(
        stderr(&output),
        format!("{}:1:2: unknown key `a`\n", nuon.display())
    );
}

#[test]
fn format_check_on_stdin_exits_with_1_and_writes_nothing_when_it_would_change() {
    let dir = TempDir::new();
    let output = format_stdin_in(&dir, &["--check", "-"], UNFORMATTED.as_bytes());
    assert_eq!(output.status.code(), Some(1), "{}", stderr(&output));
    assert!(output.stdout.is_empty());
    assert!(output.stderr.is_empty());
}

#[test]
fn format_check_on_stdin_succeeds_quietly_when_it_is_formatted() {
    let dir = TempDir::new();
    let output = format_stdin_in(&dir, &["--check", "-"], FORMATTED.as_bytes());
    assert_succeeds_quietly(&output);
    assert!(output.stdout.is_empty());
}

#[test]
fn format_rejects_stdin_with_other_paths() {
    let dir = TempDir::new();
    let [path] = write_unformatted(&dir, ["a.nu"]);
    for args in [["-", "a.nu"], ["a.nu", "-"]] {
        let output = format_stdin_in(&dir, &args, UNFORMATTED.as_bytes());
        assert_eq!(output.status.code(), Some(2));
        assert!(output.stdout.is_empty());
        assert_eq!(
            stderr(&output),
            "`-` must be the only path, since it reads stdin\n"
        );
        assert_eq!(read(&path), UNFORMATTED);
    }
}

#[test]
fn format_rejects_stdin_with_other_paths_before_reading_given_config_file() {
    let dir = TempDir::new();
    let args = ["-c", "missing.nuon", "-", "a.nu"];
    let output = format_stdin_in(&dir, &args, UNFORMATTED.as_bytes());
    assert_eq!(output.status.code(), Some(2));
    assert_eq!(
        stderr(&output),
        "`-` must be the only path, since it reads stdin\n"
    );
}

#[test]
fn format_reads_stdin_in_full_before_reporting_config_file_of_it() {
    let dir = TempDir::new();
    dir.file("wrong.nuon", "{a: 1}");
    dir.file("d/nuten.nuon", "{a: 1}");
    // More than a pipe holds, so that writing fails if the binary exits before reading it all.
    let input = "ls\n".repeat(1 << 20);
    for (args, cwd) in [
        (&["-c", "wrong.nuon", "-"][..], dir.0.clone()),
        (&["-"], dir.path("d")),
    ] {
        let mut child = nuten_command()
            .arg("format")
            .args(args)
            .current_dir(&cwd)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        let mut stdin = child.stdin.take().unwrap();
        let input = input.clone();
        let writer = thread::spawn(move || io::Write::write_all(&mut stdin, input.as_bytes()));
        let output = child.wait_with_output().unwrap();
        assert!(writer.join().unwrap().is_ok(), "{args:?}");
        assert_eq!(output.status.code(), Some(2), "{args:?}");
        assert!(output.stdout.is_empty(), "{args:?}");
    }
}

#[test]
fn format_check_on_stdin_exits_with_2_on_syntax_error() {
    let dir = TempDir::new();
    let output = format_stdin_in(&dir, &["--check", "-"], b"ls ||");
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    let stderr = stderr(&output);
    assert!(stderr.starts_with("<stdin>:1:4: "), "{stderr}");
}

#[test]
fn format_counts_stdin_formatted_as_written_when_reader_closes_first() {
    let dir = TempDir::new();
    let (reader, writer) = io::pipe().unwrap();
    drop(reader);
    let output = format_stdin_command(&dir, &["-"], UNFORMATTED.as_bytes())
        .stdout(writer)
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    assert!(output.stderr.is_empty());
}

#[cfg(target_os = "linux")]
#[test]
fn format_reports_stdin_formatted_it_cannot_write() {
    let dir = TempDir::new();
    let full = fs::File::options().write(true).open("/dev/full").unwrap();
    let output = format_stdin_command(&dir, &["-"], UNFORMATTED.as_bytes())
        .stdout(full)
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    let stderr = stderr(&output);
    assert!(stderr.starts_with("stdout: "), "{stderr}");
}

#[test]
fn format_takes_config_file_nearest_stdin_filepath_though_file_does_not_exist() {
    let dir = TempDir::new();
    dir.file("nuten.nuon", "{indent_width: 3}");
    dir.file("d/nuten.nuon", "{indent_width: 2}");
    let args = ["--stdin-filepath", "d/new.nu", "-"];
    let output = format_stdin_in(&dir, &args, BLOCK.as_bytes());
    assert_succeeds_quietly(&output);
    assert_eq!(stdout(&output), "if true {\n  ls\n}\n");
    assert!(!dir.path("d/new.nu").exists());
}

#[test]
fn format_takes_config_file_nearest_existing_directory_of_stdin_filepath() {
    let dir = TempDir::new();
    dir.file("d/nuten.nuon", "{indent_width: 2}");
    for filepath in ["d/new/new/a.nu", "d/new/../a.nu"] {
        let args = ["--stdin-filepath", filepath, "-"];
        let output = format_stdin_in(&dir, &args, BLOCK.as_bytes());
        assert_succeeds_quietly(&output);
        assert_eq!(stdout(&output), "if true {\n  ls\n}\n", "{filepath}");
        assert!(!dir.path("d/new").exists());
    }
}

#[cfg(unix)]
#[test]
fn format_takes_config_file_where_symlinked_stdin_filepath_leads() {
    let dir = TempDir::new();
    dir.file("a/nuten.nuon", "{indent_width: 2}");
    dir.file("b/nuten.nuon", "{indent_width: 3}");
    let path = dir.file("a/a.nu", BLOCK);
    std::os::unix::fs::symlink(&path, dir.path("b/link.nu")).unwrap();
    let args = ["--stdin-filepath", "b/link.nu", "-"];
    let output = format_stdin_in(&dir, &args, BLOCK.as_bytes());
    assert_succeeds_quietly(&output);
    assert_eq!(stdout(&output), "if true {\n  ls\n}\n");
    assert_eq!(read(&path), BLOCK);
}

#[test]
fn format_takes_given_config_file_over_one_nearest_stdin_filepath() {
    let dir = TempDir::new();
    dir.file("d/nuten.nuon", "{indent_width: 3}");
    dir.file("two.nuon", "{indent_width: 2}");
    let args = ["-c", "two.nuon", "--stdin-filepath", "d/a.nu", "-"];
    let output = format_stdin_in(&dir, &args, BLOCK.as_bytes());
    assert_succeeds_quietly(&output);
    assert_eq!(stdout(&output), "if true {\n  ls\n}\n");
}

#[test]
fn format_reports_syntax_error_of_stdin_at_stdin_filepath() {
    let dir = TempDir::new();
    dir.dir("d");
    let args = ["--stdin-filepath", "d/a.nu", "-"];
    let output = format_stdin_in(&dir, &args, b"ls ||");
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    let stderr = stderr(&output);
    let expected = format!("{}:1:4: ", Path::new("d/a.nu").display());
    assert!(stderr.starts_with(&expected), "{stderr}");
}

#[test]
fn format_reports_stdin_that_is_not_utf8_at_stdin_filepath() {
    let dir = TempDir::new();
    let args = ["--stdin-filepath", "d/a.nu", "-"];
    let output = format_stdin_in(&dir, &args, b"ls \xff");
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    let stderr = stderr(&output);
    let expected = format!("{}: ", Path::new("d/a.nu").display());
    assert!(stderr.starts_with(&expected), "{stderr}");
}

#[test]
fn format_check_on_stdin_takes_config_file_nearest_stdin_filepath() {
    let dir = TempDir::new();
    dir.file("d/nuten.nuon", "{indent_width: 2}");
    let args = ["--check", "--stdin-filepath", "d/a.nu", "-"];
    let output = format_stdin_in(&dir, &args, b"if true {\n  ls\n}\n");
    assert_succeeds_quietly(&output);
    assert!(output.stdout.is_empty());
}

#[test]
fn format_formats_stdin_whose_filepath_ignore_file_ignores() {
    let dir = TempDir::new();
    dir.file(".ignore", "a.nu\n");
    let args = ["--stdin-filepath", "a.nu", "-"];
    let output = format_stdin_in(&dir, &args, UNFORMATTED.as_bytes());
    assert_succeeds_quietly(&output);
    assert_eq!(stdout(&output), FORMATTED);
}

#[test]
fn format_rejects_stdin_filepath_without_stdin() {
    let dir = TempDir::new();
    let [path] = write_unformatted(&dir, ["a.nu"]);
    for args in [
        &["--stdin-filepath", "a.nu", "a.nu"][..],
        &["--stdin-filepath", "a.nu"],
    ] {
        let output = format_in(&dir, args);
        assert_eq!(output.status.code(), Some(2));
        assert_eq!(
            stderr(&output),
            "`--stdin-filepath` needs `-`, which reads stdin\n"
        );
        assert_eq!(read(&path), UNFORMATTED);
    }
}

#[test]
fn format_rejects_stdin_filepath_of_no_file() {
    let dir = TempDir::new();
    dir.dir("d");
    for filepath in ["", "d", "d/..", "/"] {
        let args = ["--stdin-filepath", filepath, "-"];
        let output = format_stdin_in(&dir, &args, UNFORMATTED.as_bytes());
        assert_eq!(output.status.code(), Some(2), "{filepath:?}");
        assert!(output.stdout.is_empty(), "{filepath:?}");
        let expected = format!("`--stdin-filepath` needs the path of a file, not `{filepath}`\n");
        assert_eq!(stderr(&output), expected);
    }
}

#[test]
fn format_rejects_stdin_filepath_before_reading_given_config_file() {
    let dir = TempDir::new();
    dir.dir("d");
    let args = ["-c", "missing.nuon", "--stdin-filepath", "a.nu", "a.nu"];
    let output = format_in(&dir, &args);
    assert_eq!(
        stderr(&output),
        "`--stdin-filepath` needs `-`, which reads stdin\n"
    );
    let args = ["-c", "missing.nuon", "--stdin-filepath", "d", "-"];
    let output = format_stdin_in(&dir, &args, UNFORMATTED.as_bytes());
    assert_eq!(
        stderr(&output),
        "`--stdin-filepath` needs the path of a file, not `d`\n"
    );
}

#[test]
fn format_takes_config_file_where_stdin_filepath_leads_through_dot_dot_after_missing_directory() {
    let dir = TempDir::new();
    dir.file("nuten.nuon", "{indent_width: 3}");
    dir.file("ok/nuten.nuon", "{indent_width: 2}");
    dir.file("wrong/nuten.nuon", "{a: 1}");
    for (filepath, expected) in [
        ("ok/nope/../../q.nu", "if true {\n   ls\n}\n"),
        ("wrong/nope/../../ok/q.nu", "if true {\n  ls\n}\n"),
        ("nope/x/../../ok/q.nu", "if true {\n  ls\n}\n"),
    ] {
        let args = ["--stdin-filepath", filepath, "-"];
        let output = format_stdin_in(&dir, &args, BLOCK.as_bytes());
        assert_succeeds_quietly(&output);
        assert_eq!(stdout(&output), expected, "{filepath}");
    }
}

#[cfg(unix)]
#[test]
fn format_takes_config_file_above_symlink_target_after_stdin_filepath_leaves_missing_directory() {
    let dir = TempDir::new();
    dir.file("nuten.nuon", "{indent_width: 3}");
    dir.file("x/nuten.nuon", "{indent_width: 2}");
    dir.file("x/y/nuten.nuon", "{indent_width: 5}");
    std::os::unix::fs::symlink(dir.path("x/y"), dir.path("link")).unwrap();
    for filepath in ["nope/../link/../q.nu", "nope/../link/sub/../../q.nu"] {
        let args = ["--stdin-filepath", filepath, "-"];
        let output = format_stdin_in(&dir, &args, BLOCK.as_bytes());
        assert_succeeds_quietly(&output);
        assert_eq!(stdout(&output), "if true {\n  ls\n}\n", "{filepath}");
    }
}

#[cfg(unix)]
#[test]
fn format_takes_config_file_beside_symlink_to_root_after_stdin_filepath_leaves_missing_directory() {
    let dir = TempDir::new();
    dir.file("nuten.nuon", "{indent_width: 2}");
    std::os::unix::fs::symlink("/", dir.path("rootlink")).unwrap();
    let args = ["--stdin-filepath", "nope/../rootlink", "-"];
    let output = format_stdin_in(&dir, &args, BLOCK.as_bytes());
    assert_succeeds_quietly(&output);
    assert_eq!(stdout(&output), "if true {\n  ls\n}\n");
}

#[test]
fn format_takes_config_file_nearest_stdin_filepath_that_leaves_current_directory() {
    let dir = TempDir::new();
    dir.file("d/nuten.nuon", "{indent_width: 2}");
    let name = dir.0.file_name().unwrap().to_str().unwrap();
    let filepath = format!("../{name}/d/new.nu");
    let args = ["--stdin-filepath", filepath.as_str(), "-"];
    let output = format_stdin_in(&dir, &args, BLOCK.as_bytes());
    assert_succeeds_quietly(&output);
    assert_eq!(stdout(&output), "if true {\n  ls\n}\n");
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
    let output = nuten_command()
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
    let output = nuten_command().arg("schema").stdout(full).output().unwrap();
    assert_eq!(output.status.code(), Some(2));
    let stderr = stderr(&output);
    assert!(stderr.starts_with("stdout: "), "{stderr}");
}
