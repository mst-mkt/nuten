//! The command-line interface.
//!
//! `nuten format` exits with 0 when every file is formatted.
//! It exits with 2 when any file has syntax errors or cannot be read or written.
//! The other files are still formatted then.
//! It formats nothing and exits with 2 when a config file is wrong or cannot be read.
//! It does the same when it cannot tell whether a config file exists.
//! The same goes for a file `--config` names without a config extension.
//! Exit code 1 is kept for a future check mode.
//! `nuten schema` exits with 2 when stdout cannot be written, unless its reader has closed it.
//! Any command exits with 2 when the arguments are invalid.

#![expect(clippy::print_stderr, reason = "errors go to the user")]

mod config;
mod files;

use std::{
    fmt::Display,
    fs,
    io::{self, Write},
    path::{Path, PathBuf},
    process::ExitCode,
};

use nuten_format::{Config, format};
use usage::{Args, Cli, Subcommands};

#[derive(Cli)]
#[usage(bin = "nuten", version, unknown_flags = "error")]
struct Nuten {
    #[usage(subcommand)]
    command: Command,
}

#[derive(Subcommands)]
enum Command {
    /// Format files in place
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
    /// Files to format in place
    #[usage(required)]
    files: Vec<PathBuf>,
}

fn main() -> ExitCode {
    match Nuten::parse().command {
        Command::Format(Format {
            config,
            files,
        }) => format_files(&files, config.as_deref()),
        Command::Schema => print_schema(),
    }
}

fn print_schema() -> ExitCode {
    match io::stdout().write_all(nuten_config::schema().as_bytes()) {
        Ok(()) => ExitCode::SUCCESS,
        // A reader that stops early, as `head` does, took what it wanted.
        Err(error) if error.kind() == io::ErrorKind::BrokenPipe => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::from(2)
        },
    }
}

/// Formats each of `files` with the config file `config_file`, or else with the one nearest it.
fn format_files(files: &[PathBuf], config_file: Option<&Path>) -> ExitCode {
    let files = files::distinct(files);
    let Ok(configs) = config::of_files(&files, config_file) else {
        return ExitCode::from(2);
    };
    let failures: Vec<_> = files
        .iter()
        .zip(&configs)
        .filter_map(|(path, config)| {
            // The problem with its config has been reported already, so it adds no line.
            let config = config.as_ref().map_err(|_| Vec::new());
            config.and_then(|config| format_file(path, config)).err()
        })
        .collect();
    for problem in failures.iter().flatten() {
        eprintln!("{problem}");
    }
    if failures.is_empty() {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(2)
    }
}

/// Formats the file `path` in place, and leaves it alone when it is formatted already.
/// The error holds a line for each problem.
fn format_file(path: &Path, config: &Config) -> Result<(), Vec<String>> {
    let io_problem = |error: io::Error| vec![format!("{}: {error}", path.display())];
    let src = read_regular_file(path).map_err(io_problem)?;
    let formatted = format(&src, config).map_err(|errors| {
        errors
            .errors
            .iter()
            .map(|error| located(path, &src, error.range.start().into(), error.kind))
            .collect::<Vec<_>>()
    })?;
    if formatted != src {
        fs::write(path, formatted).map_err(io_problem)?;
    }
    Ok(())
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
