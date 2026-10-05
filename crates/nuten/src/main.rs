#[cfg_attr(
    not(test),
    expect(dead_code, reason = "`--check` is yet to report through it")
)]
mod diff;
mod files;
mod overwrite;
mod parallel;

use std::{
    collections::{HashMap, HashSet},
    hash::Hash,
    io::{self, Write},
    panic::{self, UnwindSafe},
    path::{Path, PathBuf},
    process::ExitCode,
};

use nuten_config::{
    Config, FormatConfig, Lookup,
    file::{located, read_regular_file},
};
use nuten_format::format;
use nuten_lsp::Exit;
use usage::{Args, Cli, Subcommands, ValueEnum};

use crate::{
    files::{Candidate, Entry},
    overwrite::{Contents, Failure, OnDisk, overwrite},
};

#[derive(Cli)]
#[usage(bin = "nuten", version, unknown_flags = "error", completion)]
struct Nuten {
    #[usage(subcommand)]
    command: Command,
}

#[derive(Subcommands)]
enum Command {
    /// Format files in place or check them, or format stdin to stdout
    #[usage(
        example(
            "nuten format",
            header = "Format the `.nu` and `.nuon` files under the current directory"
        ),
        example(
            "nuten format --check",
            header = "List the files that would change, without changing them"
        ),
        example(
            "nuten format --stdin-filepath src/a.nu -",
            header = "Format stdin as the file it holds, as an editor does"
        )
    )]
    Format(Format),
    /// Print the JSON Schema of a config file
    Schema,
    /// Serve the Language Server Protocol on stdin and stdout
    Lsp(Lsp),
    /// Print the completion script for a shell
    Completions(Completions),
}

#[derive(Args)]
struct Completions {
    /// Shell the script is for
    #[usage(value_enum)]
    shell: Shell,
}

#[derive(Clone, Copy, ValueEnum)]
#[expect(
    clippy::enum_variant_names,
    reason = "`PowerShell` is the shell's own name, not a kind of `Shell`"
)]
enum Shell {
    Bash,
    Elvish,
    Fish,
    Nu,
    #[usage(name = "powershell")]
    PowerShell,
    Zsh,
}

impl From<Shell> for usage::complete::Shell {
    fn from(shell: Shell) -> Self {
        match shell {
            Shell::Bash => Self::Bash,
            Shell::Elvish => Self::Elvish,
            Shell::Fish => Self::Fish,
            Shell::Nu => Self::Nu,
            Shell::PowerShell => Self::PowerShell,
            Shell::Zsh => Self::Zsh,
        }
    }
}

#[derive(Args)]
struct Lsp {
    /// Serve on stdin and stdout, as by default; accepted for clients that pass it
    #[usage(long)]
    stdio: bool,
}

#[derive(Args)]
#[usage(args_override_self = false)]
struct Format {
    /// Config file for all files, instead of the one nearest each file
    #[usage(
        short = 'c',
        long,
        value_hint = usage::ValueHint::FilePath,
        extensions("nuon", "jsonc", "json")
    )]
    config: Option<PathBuf>,
    /// List the files that would change, instead of changing them
    #[usage(long)]
    check: bool,
    /// Path of the file stdin holds, to find its config file by and to name it in reports
    #[usage(long, value_hint = usage::ValueHint::FilePath, extensions("nu", "nuon"))]
    stdin_filepath: Option<PathBuf>,
    /// Files to format in place; a directory stands for the `.nu` and `.nuon` files under it;
    /// `-` for stdin
    #[usage(default = ".", value_hint = usage::ValueHint::FilePath, extensions("nu", "nuon"))]
    paths: Vec<PathBuf>,
}

const STDIN_PATH: &str = "-";

const STDIN_NAME: &str = "<stdin>";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    Write,
    Check,
}

#[derive(Debug)]
struct Report {
    changed: Vec<PathBuf>,
    problems: Vec<String>,
    all_excluded: bool,
}

fn main() -> ExitCode {
    match Nuten::parse().command {
        Command::Format(args) => run_format(&args),
        Command::Schema => print(&nuten_config::schema()),
        Command::Lsp(_) => serve_lsp(),
        Command::Completions(args) => print(&Nuten::completion_script(args.shell.into())),
    }
}

fn run_format(args: &Format) -> ExitCode {
    if let Some(problem) = argument_problem(args) {
        return fail(&[problem]);
    }
    let mode = if args.check { Mode::Check } else { Mode::Write };
    if reads_stdin(args) {
        let filepath = args.stdin_filepath.as_deref();
        return format_stdin(filepath, args.config.as_deref(), mode);
    }
    match args
        .config
        .as_deref()
        .map(nuten_config::load_given)
        .transpose()
    {
        Ok(given) => format_paths(&args.paths, given.as_ref(), mode),
        Err(problems) => fail(&problems),
    }
}

fn reads_stdin(args: &Format) -> bool {
    args.paths.iter().any(|path| path.as_os_str() == STDIN_PATH)
}

/// Not `conflicts` of usage, which relates arguments and cannot tell `-` from other paths.
fn argument_problem(args: &Format) -> Option<String> {
    let takes_stdin = reads_stdin(args);
    let filepath = args.stdin_filepath.as_deref();
    if takes_stdin && args.paths.len() > 1 {
        Some("`-` must be the only path, since it reads stdin".to_string())
    } else if filepath.is_some() && !takes_stdin {
        Some("`--stdin-filepath` needs `-`, which reads stdin".to_string())
    } else {
        filepath
            .filter(|file| file.file_name().is_none() || file.is_dir())
            .map(|file| {
                format!(
                    "`--stdin-filepath` needs the path of a file, not `{}`",
                    file.display()
                )
            })
    }
}

fn print(text: &str) -> ExitCode {
    match write_stdout(text) {
        Ok(()) => ExitCode::SUCCESS,
        Err(problem) => fail(&[problem]),
    }
}

/// Exits as the protocol asks.
fn serve_lsp() -> ExitCode {
    match nuten_lsp::serve_stdio() {
        Ok(Exit::AfterShutdown) => ExitCode::SUCCESS,
        Ok(Exit::WithoutShutdown) => ExitCode::from(1),
        Err(problem) => fail(&[problem]),
    }
}

fn fail(problems: &[String]) -> ExitCode {
    for problem in problems {
        write_stderr(&format!("error: {problem}"));
    }
    ExitCode::from(2)
}

/// Not a panic or a failure when stderr takes nothing, since the exit code still tells the outcome.
fn write_stderr(line: &str) {
    writeln!(io::stderr(), "{line}").ok();
}

fn write_stdout(text: &str) -> Result<(), String> {
    match io::stdout().write_all(text.as_bytes()) {
        // A reader that stops early, as `head` does, took what it wanted.
        Err(error) if error.kind() == io::ErrorKind::BrokenPipe => Ok(()),
        result => result.map_err(|error| format!("stdout: {error}")),
    }
}

fn format_stdin(filepath: Option<&Path>, config_file: Option<&Path>, mode: Mode) -> ExitCode {
    let name = filepath.unwrap_or(Path::new(STDIN_NAME));
    // Read in full first, so that a writer is not cut off when a config file has a problem.
    let src = io::read_to_string(io::stdin())
        .map_err(|error| vec![format!("{}: {error}", name.display())]);
    let formatted = src.and_then(|src| {
        let config = match config_file {
            Some(config_file) => nuten_config::load_given(config_file)?,
            None => nuten_config::of_text(filepath)?,
        };
        let format = config
            .format_of_text(filepath)
            .map_err(|problem| vec![problem])?;
        let formatted = match format {
            Some(format) => format_text(name, &src, &format)?,
            // Written back as it is, since the caller takes stdout for what the file is to hold.
            None => src.clone(),
        };
        Ok((src, formatted))
    });
    match (formatted, mode) {
        (Err(problems), _) => fail(&problems),
        (Ok((src, formatted)), Mode::Check) if formatted != src => ExitCode::from(1),
        (Ok(_), Mode::Check) => ExitCode::SUCCESS,
        (Ok((_, formatted)), Mode::Write) => print(&formatted),
    }
}

fn format_paths(paths: &[PathBuf], given: Option<&Config>, mode: Mode) -> ExitCode {
    let report = report(paths, given, mode);
    let listed = match mode {
        Mode::Write => Ok(()),
        Mode::Check => write_stdout(&listing(&report.changed)),
    };
    let problems: Vec<_> = unique(report.problems.iter())
        .cloned()
        .chain(listed.err())
        .collect();
    if !problems.is_empty() {
        fail(&problems)
    } else if mode == Mode::Check && !report.changed.is_empty() {
        ExitCode::from(1)
    } else {
        // Not a problem, since a tool may pass every file it has, whatever the config leaves out.
        if report.all_excluded {
            write_stderr("nothing to format, since `exclude` leaves out every file");
        }
        ExitCode::SUCCESS
    }
}

fn listing(paths: &[PathBuf]) -> String {
    // Lossy for names that are not UTF-8, as the lists of other formatters are.
    paths
        .iter()
        .map(|path| path.display().to_string() + "\n")
        .collect()
}

fn report(paths: &[PathBuf], given: Option<&Config>, mode: Mode) -> Report {
    let dirs = files::dirs(paths);
    let dir_lookups: HashMap<_, _> = dirs
        .iter()
        .copied()
        .zip(nuten_config::of_dirs(&dirs, given))
        .collect();
    let entries = files::expand(paths, &dir_lookups);
    let canonicals: Vec<_> = entries
        .iter()
        .filter_map(|entry| {
            let candidate = entry.as_ref().ok()?;
            Some(candidate.canonical.as_deref().map_err(String::as_str))
        })
        .collect();
    let lookups = nuten_config::of_files(&canonicals, given);
    let (taken, excluded) = taken(with_lookups(entries, lookups));
    // After the files are picked, since telling two files apart reads each.
    // Also keeps two threads off one file, which could read it halfway written.
    let jobs = files::distinct(taken, &excluded, |(candidate, _)| &candidate.path);
    let all_excluded = !excluded.is_empty() && jobs.is_empty();
    let formats = !dir_lookups
        .values()
        .chain(jobs.iter().flatten().map(|(_, lookup)| lookup))
        .any(Lookup::is_wrong);
    let outcomes = format_all(&jobs, formats.then_some(mode));
    report_of(outcomes, all_excluded)
}

type Job = Result<(Candidate, Lookup), String>;

/// `lookups` holds the config of each file of `entries`.
fn with_lookups(entries: Vec<Entry>, lookups: Vec<Lookup>) -> Vec<Job> {
    let mut lookups = lookups.into_iter();
    entries
        .into_iter()
        .map(|entry| {
            entry.map(|candidate| {
                let lookup = lookups.next().expect("a lookup for each file");
                (candidate, lookup)
            })
        })
        .collect()
}

fn taken(jobs: Vec<Job>) -> (Vec<Job>, Vec<PathBuf>) {
    let (taken, left): (Vec<_>, Vec<_>) = jobs
        .into_iter()
        .map(|job| {
            let pick = job.as_ref().map_or(Pick::Take, pick);
            (job, pick)
        })
        .partition(|(_, pick)| *pick == Pick::Take);
    let excluded = left
        .into_iter()
        .filter(|(_, pick)| *pick == Pick::Exclude)
        .filter_map(|(job, _)| Some(job.ok()?.0.path))
        .collect();
    (taken.into_iter().map(|(job, _)| job).collect(), excluded)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Pick {
    Take,
    Exclude,
    Skip,
}

/// A config with problems takes the file, so that they are reported.
fn pick((candidate, lookup): &(Candidate, Lookup)) -> Pick {
    let Lookup::Found(config) = lookup else {
        return Pick::Take;
    };
    // A file that cannot be resolved is taken, so that reading it reports why.
    let Ok(file) = &candidate.canonical else {
        return Pick::Take;
    };
    if !candidate.given && !config.includes(file) {
        Pick::Skip
    } else if config.excludes(file) {
        Pick::Exclude
    } else {
        Pick::Take
    }
}

type Outcome = Result<Option<PathBuf>, Vec<String>>;

fn format_all(jobs: &[Job], mode: Option<Mode>) -> Vec<Outcome> {
    parallel::map(jobs, |job| {
        let (candidate, lookup) = job.as_ref().map_err(|problem| vec![problem.clone()])?;
        let changed = match (lookup, mode) {
            (Lookup::Found(config), Some(mode)) => {
                let format = match &candidate.canonical {
                    Ok(file) => config.format_of(file),
                    // Reading the file reports why it cannot be resolved.
                    Err(_) => config.format.clone(),
                };
                format_file(&candidate.path, &format, mode)?
            },
            (Lookup::Found(_), None) => false,
            (lookup, _) => return Err(lookup.problems()),
        };
        Ok(changed.then(|| candidate.path.clone()))
    })
}

fn report_of(outcomes: Vec<Outcome>, all_excluded: bool) -> Report {
    Report {
        changed: outcomes
            .iter()
            .filter_map(|outcome| outcome.as_ref().ok()?.clone())
            .collect(),
        problems: outcomes
            .into_iter()
            .filter_map(Result::err)
            .flatten()
            .collect(),
        all_excluded,
    }
}

fn format_file(path: &Path, config: &FormatConfig, mode: Mode) -> Result<bool, Vec<String>> {
    let io_problem = |error: io::Error| vec![format!("{}: {error}", path.display())];
    let src = read_regular_file(path).map_err(io_problem)?;
    let formatted = format_text(path, &src, config)?;
    let changed = formatted != src;
    if changed && mode == Mode::Write {
        overwrite(&OnDisk(path), src.as_bytes(), formatted.as_bytes())
            .map_err(|failure| vec![overwrite_problem(path, &failure)])?;
    }
    Ok(changed)
}

fn overwrite_problem(path: &Path, failure: &Failure) -> String {
    let (path, error) = (path.display(), &failure.error);
    match (
        failure.contents,
        failure.backup.as_deref().map(Path::display),
    ) {
        (Contents::New, Some(backup)) => {
            format!("{backup}: {error}; {path} is formatted, but this backup of it is left behind")
        },
        (Contents::Broken, Some(backup)) => {
            format!("{path}: {error}; its old contents are in {backup}")
        },
        (Contents::Broken, None) => format!("{path}: {error}; it may be left partly written"),
        (Contents::Old, Some(backup)) => format!("{path}: {error}; {backup} is left behind"),
        (_, None) => format!("{path}: {error}"),
    }
}

fn format_text(path: &Path, src: &str, config: &FormatConfig) -> Result<String, Vec<String>> {
    catch_panic(path, || {
        format(src, config).map_err(|errors| {
            located(
                path,
                src,
                errors
                    .errors
                    .iter()
                    .map(|error| (error.range.start().into(), error.kind)),
            )
        })
    })?
}

/// Not left to end the run, which would lose the report of the other files.
fn catch_panic<T>(path: &Path, f: impl FnOnce() -> T + UnwindSafe) -> Result<T, Vec<String>> {
    panic::catch_unwind(f).map_err(|_| {
        vec![format!(
            "{}: nuten panicked while formatting",
            path.display()
        )]
    })
}

fn unique<T: Copy + Eq + Hash>(items: impl Iterator<Item = T>) -> impl Iterator<Item = T> {
    let mut seen = HashSet::new();
    items.filter(move |item| seen.insert(*item))
}

#[cfg(test)]
mod tests {
    use std::{
        io,
        path::{Path, PathBuf},
    };

    use super::{catch_panic, overwrite_problem};
    use crate::overwrite::{Contents, Failure};

    #[test]
    fn catch_panic_turns_panic_into_problem_naming_file() {
        assert_eq!(
            catch_panic(Path::new("dir/a.nu"), || panic!("bug")),
            Err::<(), _>(vec![
                "dir/a.nu: nuten panicked while formatting".to_string()
            ])
        );
    }

    #[test]
    fn catch_panic_passes_on_value_of_f() {
        assert_eq!(catch_panic(Path::new("dir/a.nu"), || 1), Ok(1));
    }

    #[test]
    fn overwrite_problem_says_what_file_and_backup_hold() {
        let cases = [
            (Contents::Old, None, "a.nu: e"),
            (Contents::Old, Some("b"), "a.nu: e; b is left behind"),
            (
                Contents::New,
                Some("b"),
                "b: e; a.nu is formatted, but this backup of it is left behind",
            ),
            (
                Contents::Broken,
                Some("b"),
                "a.nu: e; its old contents are in b",
            ),
            (
                Contents::Broken,
                None,
                "a.nu: e; it may be left partly written",
            ),
        ];
        for (contents, backup, expected) in cases {
            let failure = Failure {
                error: io::Error::other("e"),
                contents,
                backup: backup.map(PathBuf::from),
            };
            assert_eq!(overwrite_problem(Path::new("a.nu"), &failure), expected);
        }
    }
}
