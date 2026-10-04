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

/// Its path has no symlinks, since the paths of config files are reported with symlinks resolved.
pub(crate) struct TempDir(pub(crate) PathBuf);

impl TempDir {
    pub(crate) fn new() -> TempDir {
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

    pub(crate) fn path(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }

    pub(crate) fn dir(&self, name: &str) -> PathBuf {
        let path = self.path(name);
        fs::create_dir_all(&path).unwrap();
        path
    }

    pub(crate) fn file(&self, name: &str, contents: &str) -> PathBuf {
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

/// Blind to the git config of the user, which could ignore the files of a test.
pub(crate) fn nuten_command() -> Command {
    let nowhere = env::temp_dir().join("nuten-cli-nowhere");
    let mut command = Command::new(env!("CARGO_BIN_EXE_nuten"));
    command
        .env("HOME", &nowhere)
        .env("XDG_CONFIG_HOME", &nowhere)
        .env("GIT_CONFIG_SYSTEM", &nowhere)
        .env_remove("GIT_CONFIG_GLOBAL");
    command
}

pub(crate) fn nuten(args: &[&OsStr]) -> Output {
    nuten_command().args(args).output().unwrap()
}

pub(crate) fn format(paths: &[&Path]) -> Output {
    let args: Vec<_> = [OsStr::new("format")]
        .into_iter()
        .chain(paths.iter().map(|path| path.as_os_str()))
        .collect();
    nuten(&args)
}

pub(crate) fn format_in(dir: &TempDir, args: &[&str]) -> Output {
    nuten_command()
        .arg("format")
        .args(args)
        .current_dir(&dir.0)
        .output()
        .unwrap()
}

/// Not a pipe, whose writer fails when the binary exits before it reads stdin.
pub(crate) fn format_stdin_command(dir: &TempDir, args: &[&str], input: &[u8]) -> Command {
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

pub(crate) fn format_stdin_in(dir: &TempDir, args: &[&str], input: &[u8]) -> Output {
    format_stdin_command(dir, args, input).output().unwrap()
}

/// Each indent width formats a block differently.
pub(crate) const BLOCK: &str = "if true {\nls\n}";

pub(crate) fn read(path: &Path) -> String {
    fs::read_to_string(path).unwrap()
}

pub(crate) fn stderr(output: &Output) -> String {
    String::from_utf8(output.stderr.clone()).unwrap()
}

pub(crate) fn stdout(output: &Output) -> String {
    String::from_utf8(output.stdout.clone()).unwrap()
}

pub(crate) fn io_error_line(path: &Path, error: &io::Error) -> String {
    format!("{}: {error}\n", path.display())
}

#[cfg(unix)]
pub(crate) fn fifo(path: &Path) {
    let status = Command::new("mkfifo").arg(path).status().unwrap();
    assert!(status.success());
}

/// Not `format`, which would hang along with the binary if it read a FIFO.
#[cfg(unix)]
pub(crate) fn format_in_time(paths: &[&Path]) -> Output {
    output_in_time(nuten_command().arg("format").args(paths))
}

/// Not `Command::output`, which would hang along with the binary.
pub(crate) fn output_in_time(command: &mut Command) -> Output {
    let mut child = command
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(10);
    while child.try_wait().unwrap().is_none() {
        if Instant::now() > deadline {
            child.kill().unwrap();
            panic!("the binary runs for too long");
        }
        thread::sleep(Duration::from_millis(10));
    }
    child.wait_with_output().unwrap()
}

pub(crate) const UNFORMATTED: &str = "ls|first";
pub(crate) const FORMATTED: &str = "ls | first\n";

pub(crate) fn write_unformatted<const N: usize>(dir: &TempDir, names: [&str; N]) -> [PathBuf; N] {
    names.map(|name| dir.file(name, UNFORMATTED))
}

pub(crate) fn assert_succeeds_quietly(output: &Output) {
    assert_eq!(output.status.code(), Some(0), "{}", stderr(output));
    assert!(output.stderr.is_empty(), "{}", stderr(output));
}
