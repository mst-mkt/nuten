mod diff;
mod files;
mod overwrite;
mod parallel;
mod problem;
mod reporter;
mod style;
mod summary;
mod view;

use std::{
    collections::{HashMap, HashSet},
    fs,
    hash::Hash,
    io::{self, Write},
    panic::{self, UnwindSafe},
    path::{Path, PathBuf},
    process::ExitCode,
};

use nuten_config::{Config, FormatConfig, Lookup, LookupError};
use nuten_format::format;
use nuten_line_index::located;
use nuten_lsp::Exit;
use usage::{Args, Cli, Subcommands, ValueEnum};

use crate::{
    diff::Difference,
    files::{Candidate, Entry},
    overwrite::{Contents, Failure, OnDisk, overwrite},
    problem::Problem,
    reporter::{github, gitlab},
    style::Style,
    summary::{Counts, summary},
    view::view,
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
            header = "Show what would change, without changing any file"
        ),
        example(
            "nuten format --reporter github",
            header = "Annotate the lines that would change in a pull request, on GitHub Actions"
        ),
        example(
            "nuten format --reporter gitlab",
            header = "Write a Code Quality report for merge requests to stdout, on GitLab CI"
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
    /// Show what would change, without changing any file
    #[usage(long)]
    check: bool,
    /// List the files that would change and nothing else, for a script to read
    #[usage(short = 'l', long, conflicts("check"))]
    list_different: bool,
    /// Report what would change and every problem in a form a CI service reads, changing no file
    #[usage(long, value_enum, conflicts("check", "list_different"))]
    reporter: Option<Reporter>,
    /// Path of the file stdin holds, to find its config file by and to name it in reports
    #[usage(long, value_hint = usage::ValueHint::FilePath, extensions("nu", "nuon"))]
    stdin_filepath: Option<PathBuf>,
    /// Files to format in place; a directory stands for the `.nu` and `.nuon` files under it;
    /// `-` for stdin
    #[usage(default = ".", value_hint = usage::ValueHint::FilePath, extensions("nu", "nuon"))]
    paths: Vec<PathBuf>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum Reporter {
    /// Annotations of GitHub Actions
    Github,
    /// Code Quality report of GitLab CI
    Gitlab,
}

const STDIN_PATH: &str = "-";

const STDIN_NAME: &str = "<stdin>";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    Write,
    Check,
    List,
    Report(Reporter),
}

#[derive(Debug)]
struct Report {
    changed: Vec<Changed>,
    problems: Vec<Problem>,
    counts: Counts,
}

#[derive(Debug)]
struct Changed {
    path: PathBuf,
    /// Only for the modes that tell how a file changes.
    detail: Option<Detail>,
}

#[derive(Debug)]
struct Detail {
    text: String,
    changed_lines: usize,
    whitespace_only_lines: usize,
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
    let mode = match (args.check, args.list_different, args.reporter) {
        (_, _, Some(reporter)) => Mode::Report(reporter),
        (true, _, _) => Mode::Check,
        (_, true, _) => Mode::List,
        _ => Mode::Write,
    };
    if let Some(problem) = argument_problem(args) {
        return finish(&[], vec![problem], mode, Style::stdout());
    }
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
        Err(errors) => finish(&[], problems_of(errors), mode, Style::stdout()),
    }
}

fn reads_stdin(args: &Format) -> bool {
    args.paths.iter().any(|path| path.as_os_str() == STDIN_PATH)
}

/// Not `conflicts` of usage, which relates arguments and cannot tell `-` from other paths.
fn argument_problem(args: &Format) -> Option<Problem> {
    let takes_stdin = reads_stdin(args);
    let filepath = args.stdin_filepath.as_deref();
    if takes_stdin && args.paths.len() > 1 {
        Some(Problem::new(
            "`-` must be the only path, since it reads stdin",
        ))
    } else if filepath.is_some() && !takes_stdin {
        Some(Problem::new(
            "`--stdin-filepath` needs `-`, which reads stdin",
        ))
    } else if args.reporter.is_some() && takes_stdin && filepath.is_none() {
        Some(Problem::new(
            "`--reporter` needs `--stdin-filepath` on stdin, which names the file to report on",
        ))
    } else {
        filepath
            .filter(|file| file.file_name().is_none() || file.is_dir())
            .map(|file| {
                Problem::new(format!(
                    "`--stdin-filepath` needs the path of a file, not `{}`",
                    file.display()
                ))
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
        Err(problem) => fail(&[Problem::new(problem)]),
    }
}

fn fail(problems: &[Problem]) -> ExitCode {
    let error = Style::stderr().error("error:");
    for problem in problems {
        write_stderr(&format!("{error} {problem}"));
    }
    ExitCode::from(2)
}

/// Not a panic or a failure when stderr takes nothing, since the exit code still tells the outcome.
fn write_stderr(line: &str) {
    writeln!(io::stderr(), "{line}").ok();
}

fn write_stdout(text: &str) -> Result<(), Problem> {
    match io::stdout().write_all(text.as_bytes()) {
        // A reader that stops early, as `head` does, took what it wanted.
        Err(error) if error.kind() == io::ErrorKind::BrokenPipe => Ok(()),
        result => result.map_err(|error| Problem::new(format!("stdout: {error}"))),
    }
}

fn format_stdin(filepath: Option<&Path>, config_file: Option<&Path>, mode: Mode) -> ExitCode {
    let name = filepath.unwrap_or(Path::new(STDIN_NAME));
    // Read in full first, so that a writer is not cut off when a config file has a problem.
    let src = io::read_to_string(io::stdin()).map_err(|error| vec![Problem::in_file(name, error)]);
    let formatted = src.and_then(|src| {
        let config = match config_file {
            Some(config_file) => nuten_config::load_given(config_file),
            None => nuten_config::of_text(filepath),
        }
        .map_err(problems_of)?;
        let format = config
            .format_of_text(filepath)
            .map_err(|error| vec![error.into()])?;
        let formatted = match format {
            Some(format) => format_text(name, &src, &format)?,
            // Written back as it is, since the caller takes stdout for what the file is to hold.
            None => src.clone(),
        };
        Ok((src, formatted))
    });
    let style = Style::stdout();
    let (changed, problems) = match formatted {
        Ok((_, formatted)) if mode == Mode::Write => return print(&formatted),
        Ok((src, formatted)) => {
            let changed = (formatted != src).then(|| Changed {
                path: name.to_path_buf(),
                detail: detail(name, &src, &formatted, mode, style),
            });
            (changed.into_iter().collect(), Vec::new())
        },
        Err(problems) => (Vec::new(), problems),
    };
    finish(&changed, problems, mode, style)
}

fn format_paths(paths: &[PathBuf], given: Option<&Config>, mode: Mode) -> ExitCode {
    let style = Style::stdout();
    let report = report(paths, given, mode, style);
    let problems = unique(report.problems.iter()).cloned().collect();
    let code = finish(&report.changed, problems, mode, style);
    if let Some(summary) = summary(&report.counts, mode, Style::stderr()) {
        write_stderr(&summary);
    }
    code
}

fn finish(changed: &[Changed], problems: Vec<Problem>, mode: Mode, style: Style) -> ExitCode {
    let written = write_stdout(&stdout_text(changed, &problems, mode, style));
    let problems: Vec<_> = problems.into_iter().chain(written.err()).collect();
    if !problems.is_empty() {
        fail(&problems)
    } else if mode != Mode::Write && !changed.is_empty() {
        ExitCode::from(1)
    } else {
        ExitCode::SUCCESS
    }
}

/// A reporter writes problems too, since what reads it reads only stdout.
fn stdout_text(changed: &[Changed], problems: &[Problem], mode: Mode, style: Style) -> String {
    match mode {
        Mode::Write => String::new(),
        Mode::Check => views_text(changed, style),
        Mode::List => listing(changed.iter().map(|changed| changed.path.as_path())),
        Mode::Report(Reporter::Github) => {
            detail_texts(changed).collect::<String>()
                + &problems
                    .iter()
                    .map(github::problem_annotation)
                    .collect::<String>()
        },
        Mode::Report(Reporter::Gitlab) => gitlab::report(
            detail_texts(changed)
                .map(str::to_string)
                .chain(problems.iter().filter_map(gitlab::problem_issue)),
        ),
    }
}

fn detail_texts(changed: &[Changed]) -> impl Iterator<Item = &str> {
    changed
        .iter()
        .filter_map(|changed| Some(changed.detail.as_ref()?.text.as_str()))
}

fn views_text(changed: &[Changed], style: Style) -> String {
    if changed.is_empty() {
        return String::new();
    }
    heading(changed.len(), style) + &detail_texts(changed).collect::<String>()
}

fn heading(count: usize, style: Style) -> String {
    let files = if count == 1 { "file" } else { "files" };
    style.bold(&format!("{count} {files} would be reformatted:")) + "\n\n"
}

fn listing<'a>(paths: impl IntoIterator<Item = &'a Path>) -> String {
    // Lossy for names that are not UTF-8, as the lists of other formatters are.
    paths
        .into_iter()
        .map(|path| path.display().to_string() + "\n")
        .collect()
}

fn report(paths: &[PathBuf], given: Option<&Config>, mode: Mode, style: Style) -> Report {
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
            Some(candidate.canonical.as_deref())
        })
        .collect();
    let lookups = nuten_config::of_files(&canonicals, given);
    let (taken, excluded) = taken(with_lookups(entries, lookups));
    // After the files are picked, since telling two files apart reads each.
    // Also keeps two threads off one file, which could read it halfway written.
    let jobs = files::distinct(taken, &excluded, |(candidate, _)| &candidate.path);
    // Not one per path, since a file given under two names is excluded once.
    let excluded_count =
        files::distinct(excluded.into_iter().map(Ok).collect(), &[], |path| path).len();
    let formats = !dir_lookups
        .values()
        .chain(jobs.iter().flatten().map(|(_, lookup)| lookup))
        .any(Lookup::is_wrong);
    let outcomes = format_all(&jobs, formats.then_some(mode), style);
    report_of(outcomes, excluded_count)
}

type Job = Result<(Candidate, Lookup), Problem>;

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

#[derive(Debug)]
enum Outcome {
    Changed(Changed),
    Unchanged,
    /// Left alone, since some config file is wrong.
    Skipped(Vec<Problem>),
    Failed(Vec<Problem>),
    /// Of no file, such as a line of an ignore file.
    Unattached(Vec<Problem>),
}

/// Takes `style`, since the detail of each file builds here in parallel.
fn format_all(jobs: &[Job], mode: Option<Mode>, style: Style) -> Vec<Outcome> {
    parallel::map(jobs, |job| {
        let (candidate, lookup) = match job {
            Ok(found) => found,
            Err(problem) => return Outcome::Unattached(vec![problem.clone()]),
        };
        match (lookup, mode) {
            (Lookup::Found(config), Some(mode)) => {
                let format = match &candidate.canonical {
                    Ok(file) => config.format_of(file),
                    // Reading the file reports why it cannot be resolved.
                    Err(_) => config.format_without_file(),
                };
                match format_file(&candidate.path, &format, mode, style) {
                    Ok(Some(changed)) => Outcome::Changed(changed),
                    Ok(None) => Outcome::Unchanged,
                    Err(problems) => Outcome::Failed(problems),
                }
            },
            (Lookup::Found(_), None) => Outcome::Skipped(Vec::new()),
            (Lookup::Unresolved(error), _) => Outcome::Failed(vec![error.clone().into()]),
            (Lookup::Wrong(errors), _) => Outcome::Skipped(problems_of(errors.clone())),
        }
    })
}

fn report_of(outcomes: Vec<Outcome>, excluded: usize) -> Report {
    let count =
        |kind: fn(&Outcome) -> bool| outcomes.iter().filter(|outcome| kind(outcome)).count();
    let details = || {
        outcomes.iter().filter_map(|outcome| match outcome {
            Outcome::Changed(changed) => changed.detail.as_ref(),
            _ => None,
        })
    };
    let counts = Counts {
        changed: count(|outcome| matches!(outcome, Outcome::Changed(_))),
        unchanged: count(|outcome| matches!(outcome, Outcome::Unchanged)),
        failed: count(|outcome| matches!(outcome, Outcome::Failed(_))),
        skipped: count(|outcome| matches!(outcome, Outcome::Skipped(_))),
        excluded,
        changed_lines: details().map(|detail| detail.changed_lines).sum(),
        whitespace_only_lines: details().map(|detail| detail.whitespace_only_lines).sum(),
    };
    let (changed, problems): (Vec<_>, Vec<_>) = outcomes
        .into_iter()
        .filter_map(|outcome| match outcome {
            Outcome::Changed(changed) => Some(Ok(changed)),
            Outcome::Unchanged => None,
            Outcome::Skipped(problems)
            | Outcome::Failed(problems)
            | Outcome::Unattached(problems) => Some(Err(problems)),
        })
        .partition(Result::is_ok);
    let changed = changed.into_iter().filter_map(Result::ok).collect();
    let problems = problems
        .into_iter()
        .filter_map(Result::err)
        .flatten()
        .collect();
    Report {
        changed,
        problems,
        counts,
    }
}

fn format_file(
    path: &Path,
    config: &FormatConfig,
    mode: Mode,
    style: Style,
) -> Result<Option<Changed>, Vec<Problem>> {
    let io_problem = |error: io::Error| vec![Problem::in_file(path, error)];
    let src = read_regular_file(path).map_err(io_problem)?;
    let formatted = format_text(path, &src, config)?;
    if formatted == src {
        return Ok(None);
    }
    if mode == Mode::Write {
        overwrite(&OnDisk(path), src.as_bytes(), formatted.as_bytes())
            .map_err(|failure| vec![overwrite_problem(path, &failure)])?;
    }
    Ok(Some(Changed {
        path: path.to_path_buf(),
        detail: detail(path, &src, &formatted, mode, style),
    }))
}

fn detail(path: &Path, src: &str, formatted: &str, mode: Mode, style: Style) -> Option<Detail> {
    let text_of: fn(&Path, &Difference<'_>, Style) -> String = match mode {
        Mode::Write | Mode::List => return None,
        Mode::Check => view,
        Mode::Report(Reporter::Github) => {
            |path, difference, _| github::file_annotation(path, difference)
        },
        Mode::Report(Reporter::Gitlab) => {
            |path, difference, _| gitlab::file_issue(path, difference)
        },
    };
    let difference = Difference::new(src, formatted);
    Some(Detail {
        text: text_of(path, &difference, style),
        changed_lines: difference.changed_lines(),
        whitespace_only_lines: difference.whitespace_only_lines(),
    })
}

/// Not `fs::read_to_string` alone, which waits forever on a FIFO.
fn read_regular_file(path: &Path) -> io::Result<String> {
    if fs::metadata(path)?.is_file() {
        fs::read_to_string(path)
    } else {
        Err(io::Error::other("not a regular file"))
    }
}

fn overwrite_problem(path: &Path, failure: &Failure) -> Problem {
    let error = &failure.error;
    match (failure.contents, failure.backup.as_deref()) {
        (Contents::New, Some(backup)) => Problem::in_file(
            backup,
            format!(
                "{error}; {} is formatted, but this backup of it is left behind",
                path.display()
            ),
        ),
        (Contents::Broken, Some(backup)) => Problem::in_file(
            path,
            format!("{error}; its old contents are in {}", backup.display()),
        ),
        (Contents::Broken, None) => {
            Problem::in_file(path, format!("{error}; it may be left partly written"))
        },
        (Contents::Old, Some(backup)) => Problem::in_file(
            path,
            format!("{error}; {} is left behind", backup.display()),
        ),
        (_, None) => Problem::in_file(path, error),
    }
}

fn format_text(path: &Path, src: &str, config: &FormatConfig) -> Result<String, Vec<Problem>> {
    catch_panic(path, || {
        format(src, config).map_err(|errors| {
            located(
                src,
                errors
                    .errors
                    .iter()
                    .map(|error| (error.range.start().into(), error.kind)),
            )
            .into_iter()
            .map(|(line_column, kind)| Problem::at(path, line_column.into(), kind))
            .collect()
        })
    })?
}

/// Not left to end the run, which would lose the report of the other files.
fn catch_panic<T>(path: &Path, f: impl FnOnce() -> T + UnwindSafe) -> Result<T, Vec<Problem>> {
    panic::catch_unwind(f)
        .map_err(|_| vec![Problem::in_file(path, "nuten panicked while formatting")])
}

fn problems_of(errors: Vec<LookupError>) -> Vec<Problem> {
    errors.into_iter().map(Problem::from).collect()
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

    use nuten_config::FormatConfig;

    use super::{
        Changed, Detail, Outcome, Problem, catch_panic, format_text, overwrite_problem, report_of,
        views_text,
    };
    use crate::{
        overwrite::{Contents, Failure},
        problem::Position,
        style::Style,
    };

    #[test]
    fn format_text_places_each_syntax_error_at_its_line_and_column() {
        let problems =
            format_text(Path::new("a.nu"), "ls\nls ||", &FormatConfig::default()).unwrap_err();
        let places: Vec<_> = problems
            .into_iter()
            .map(|problem| (problem.path, problem.position))
            .collect();
        let position = Position {
            line: 2,
            column: Some(4),
        };
        assert_eq!(places, [(Some(PathBuf::from("a.nu")), Some(position))]);
    }

    #[test]
    fn catch_panic_turns_panic_into_problem_naming_file() {
        assert_eq!(
            catch_panic(Path::new("dir/a.nu"), || panic!("bug")),
            Err::<(), _>(vec![Problem::in_file(
                Path::new("dir/a.nu"),
                "nuten panicked while formatting"
            )])
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
            assert_eq!(
                overwrite_problem(Path::new("a.nu"), &failure).to_string(),
                expected
            );
        }
    }

    fn changed(name: &str) -> Changed {
        Changed {
            path: PathBuf::from(name),
            detail: Some(Detail {
                text: format!("{name}\n"),
                changed_lines: 3,
                whitespace_only_lines: 1,
            }),
        }
    }

    #[test]
    fn report_of_sums_lines_of_each_view() {
        let outcomes = vec![
            Outcome::Changed(changed("a")),
            Outcome::Unchanged,
            Outcome::Changed(changed("b")),
        ];
        let counts = report_of(outcomes, 0).counts;
        assert_eq!((counts.changed_lines, counts.whitespace_only_lines), (6, 2));
    }

    #[test]
    fn views_text_heads_views_with_count_in_bold() {
        assert_eq!(
            views_text(&[changed("a")], Style::COLORED),
            "\u{1b}[1m1 file would be reformatted:\u{1b}[0m\n\na\n"
        );
        assert_eq!(
            views_text(&[changed("a"), changed("b")], Style::PLAIN),
            "2 files would be reformatted:\n\na\nb\n"
        );
        assert_eq!(views_text(&[], Style::PLAIN), "");
    }
}
