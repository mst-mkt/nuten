//! The command-line interface.
//!
//! `nuten format` exits with 0 when every file is formatted.
//! It exits with 2 when any file has syntax errors or cannot be read or written.
//! It does the same when a directory it walks into cannot be read.
//! The same goes for an ignore file with a line it cannot read.
//! The other files are still formatted then.
//! It formats nothing and exits with 2 when a config file is wrong or cannot be read.
//! It does the same when it cannot tell whether a config file exists.
//! The same goes for a file `--config` names without a config extension.
//! With `--check`, it changes no file.
//! It then exits with 1 when any file would change and nothing is wrong.
//! Any command exits with 2 when stdout cannot be written, unless its reader has closed it.
//! Any command exits with 2 when the arguments are invalid.

#![expect(clippy::print_stderr, reason = "errors go to the user")]

mod config;
mod files;
mod parallel;

use std::{
    collections::{HashMap, HashSet},
    fmt::Display,
    fs,
    hash::Hash,
    io::{self, Write},
    path::{Path, PathBuf},
    process::ExitCode,
};

use nuten_config::{Config, FormatConfig};
use nuten_format::format;
use usage::{Args, Cli, Subcommands};

use crate::{config::Lookup, files::Entry};

#[derive(Cli)]
#[usage(bin = "nuten", version, unknown_flags = "error")]
struct Nuten {
    #[usage(subcommand)]
    command: Command,
}

#[derive(Subcommands)]
enum Command {
    /// Format files in place, or check whether they are formatted
    Format(Format),
    /// Print the JSON Schema of a config file
    Schema,
}

#[derive(Args)]
#[usage(args_override_self = false)]
struct Format {
    /// Config file for all files, instead of the one nearest each file
    #[usage(short = 'c', long)]
    config: Option<PathBuf>,
    /// List the files that would change, instead of changing them
    #[usage(long)]
    check: bool,
    /// Files to format in place; a directory stands for the `.nu` files under it
    #[usage(default = ".")]
    paths: Vec<PathBuf>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    Write,
    Check,
}

/// What formatting the files comes to.
#[derive(Debug)]
struct Report {
    /// The files formatted differently from how they are written, in the order of the paths given.
    changed: Vec<PathBuf>,
    /// A line for each problem, at each path given that it concerns, in the order of the paths.
    problems: Vec<String>,
}

fn main() -> ExitCode {
    match Nuten::parse().command {
        Command::Format(Format {
            config,
            check,
            paths,
        }) => {
            let mode = if check { Mode::Check } else { Mode::Write };
            format_paths(&paths, config.as_deref(), mode)
        },
        Command::Schema => print_schema(),
    }
}

fn print_schema() -> ExitCode {
    match write_stdout(&nuten_config::schema()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(problem) => fail(&[problem]),
    }
}

/// Reports `problems` to stderr.
fn fail(problems: &[String]) -> ExitCode {
    for problem in problems {
        eprintln!("{problem}");
    }
    ExitCode::from(2)
}

/// Writes `text` to stdout.
/// The error is the problem, as a line.
fn write_stdout(text: &str) -> Result<(), String> {
    match io::stdout().write_all(text.as_bytes()) {
        // A reader that stops early, as `head` does, took what it wanted.
        Err(error) if error.kind() == io::ErrorKind::BrokenPipe => Ok(()),
        result => result.map_err(|error| format!("stdout: {error}")),
    }
}

/// Formats the files `paths` name, with the config file `config_file` or else the nearest one.
fn format_paths(paths: &[PathBuf], config_file: Option<&Path>, mode: Mode) -> ExitCode {
    let report = match config_file.map(config::load).transpose() {
        Ok(given) => report(paths, given.as_ref(), mode),
        Err(problems) => Report {
            changed: Vec::new(),
            problems,
        },
    };
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
        ExitCode::SUCCESS
    }
}

/// `paths`, a line each.
fn listing(paths: &[PathBuf]) -> String {
    // Lossy for names that are not UTF-8, as the lists of other formatters are.
    paths
        .iter()
        .map(|path| path.display().to_string() + "\n")
        .collect()
}

/// What formatting the files `paths` name comes to, with `given` or else the nearest config.
fn report(paths: &[PathBuf], given: Option<&Config>, mode: Mode) -> Report {
    let dirs = files::dirs(paths);
    let dir_lookups: HashMap<_, _> = dirs
        .iter()
        .copied()
        .zip(config::of_dirs(&dirs, given))
        .collect();
    // Also keeps two threads off one file, which could read it halfway written.
    let entries = files::distinct(files::expand(paths, &dir_lookups));
    let files: Vec<_> = entries
        .iter()
        .filter_map(|entry| entry.as_deref().ok())
        .collect();
    let lookups = config::of_files(&files, given);
    let formats = !dir_lookups.values().chain(&lookups).any(Lookup::is_wrong);
    let outcomes = format_all(&files, &lookups, formats.then_some(mode));
    report_of(entries, outcomes)
}

/// Whether each of `files` changes, or else a line for each problem with it or its config.
/// Formats nothing when `mode` is `None`.
fn format_all(
    files: &[&Path],
    lookups: &[Lookup],
    mode: Option<Mode>,
) -> Vec<Result<bool, Vec<String>>> {
    let jobs: Vec<_> = files.iter().zip(lookups).collect();
    parallel::map(&jobs, |(path, lookup)| match (lookup, mode) {
        (Lookup::Found(config), Some(mode)) => format_file(path, &config.format, mode),
        (Lookup::Found(_), None) => Ok(false),
        (lookup, _) => Err(lookup.problems()),
    })
}

/// What `entries` come to, where what formatting each file comes to is next in `outcomes`.
fn report_of(entries: Vec<Entry>, outcomes: Vec<Result<bool, Vec<String>>>) -> Report {
    let mut outcomes = outcomes.into_iter();
    let entry_outcomes: Vec<_> = entries
        .into_iter()
        .map(|entry| match entry {
            Ok(path) => outcomes
                .next()
                .expect("an outcome for each file")
                .map(|changed| changed.then_some(path)),
            Err(problem) => Err(vec![problem]),
        })
        .collect();
    Report {
        changed: entry_outcomes
            .iter()
            .filter_map(|result| result.as_ref().ok()?.clone())
            .collect(),
        problems: entry_outcomes
            .into_iter()
            .filter_map(Result::err)
            .flatten()
            .collect(),
    }
}

/// Formats the file `path`, and writes it back when `mode` says to and it changes.
/// Tells whether it changes.
/// The error holds a line for each problem.
fn format_file(path: &Path, config: &FormatConfig, mode: Mode) -> Result<bool, Vec<String>> {
    let io_problem = |error: io::Error| vec![format!("{}: {error}", path.display())];
    let src = read_regular_file(path).map_err(io_problem)?;
    let formatted = format(&src, config).map_err(|errors| {
        errors
            .errors
            .iter()
            .map(|error| located(path, &src, error.range.start().into(), error.kind))
            .collect::<Vec<_>>()
    })?;
    let changed = formatted != src;
    if changed && mode == Mode::Write {
        fs::write(path, formatted).map_err(io_problem)?;
    }
    Ok(changed)
}

/// Each of `items` once, in the order it first comes.
fn unique<T: Copy + Eq + Hash>(items: impl Iterator<Item = T>) -> impl Iterator<Item = T> {
    let mut seen = HashSet::new();
    items.filter(move |item| seen.insert(*item))
}

/// Not `fs::read_to_string` alone, which waits forever on a FIFO.
fn read_regular_file(path: &Path) -> io::Result<String> {
    if fs::metadata(path)?.is_file() {
        fs::read_to_string(path)
    } else {
        Err(io::Error::other("not a regular file"))
    }
}

/// `message` headed by the line and column of `offset` in `src`, the text of `path`.
fn located(path: &Path, src: &str, offset: usize, message: impl Display) -> String {
    let (line, column) = line_column(src, offset);
    format!("{}:{line}:{column}: {message}", path.display())
}

/// 1-based, the column in characters.
fn line_column(src: &str, offset: usize) -> (usize, usize) {
    let before = &src[..offset];
    let line = before.matches('\n').count() + 1;
    let last_line = before.rsplit_once('\n').map_or(before, |(_, last)| last);
    let column = last_line.chars().count() + 1;
    (line, column)
}

#[cfg(test)]
mod tests {
    use super::line_column;

    #[test]
    fn line_column_is_one_based_in_characters() {
        assert_eq!(line_column("ls |", 3), (1, 4));
        assert_eq!(line_column("ls\n\nls |", 7), (3, 4));
        assert_eq!(line_column("ls\r\nls |", 7), (2, 4));
        assert_eq!(line_column("echo 日本 |", 12), (1, 9));
        assert_eq!(line_column("", 0), (1, 1));
    }
}
