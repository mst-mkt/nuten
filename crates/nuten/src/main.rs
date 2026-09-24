//! The command-line interface.
//!
//! `nuten format` exits with 0 when every file is formatted.
//! It exits with 2 when any file has syntax errors or cannot be read or written.
//! The other files are still formatted then.
//! It formats nothing and exits with 2 when a config file is wrong or cannot be read.
//! It does the same when it cannot tell whether a config file exists.
//! Exit code 1 is kept for a future check mode.
//! `nuten schema` exits with 2 when stdout cannot be written, unless its reader has closed it.
//! Any command exits with 2 when the arguments are invalid.

#![expect(clippy::print_stderr, reason = "errors go to the user")]

mod config;

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
struct Format {
    /// Files to format in place
    #[usage(required)]
    files: Vec<PathBuf>,
}

fn main() -> ExitCode {
    match Nuten::parse().command {
        Command::Format(Format {
            files,
        }) => format_files(&files),
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

/// Formats each of `files` with the config file nearest it.
fn format_files(files: &[PathBuf]) -> ExitCode {
    let Ok(configs) = config::nearest(files) else {
        return ExitCode::from(2);
    };
    // Not `any`: the files after a failure are still to be formatted.
    let failures = files
        .iter()
        .zip(&configs)
        .filter(|(path, config)| {
            !config
                .as_ref()
                .is_ok_and(|config| format_file(path, config))
        })
        .count();
    if failures == 0 {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(2)
    }
}

/// Reports every problem to stderr; returns whether the file is formatted.
/// Leaves the file alone when it is formatted already.
fn format_file(path: &Path, config: &Config) -> bool {
    let src = match fs::read_to_string(path) {
        Ok(src) => src,
        Err(error) => {
            eprintln!("{}: {error}", path.display());
            return false;
        },
    };
    let formatted = match format(&src, config) {
        Ok(formatted) => formatted,
        Err(errors) => {
            for error in &errors.errors {
                report_at(path, &src, error.range.start().into(), error.kind);
            }
            return false;
        },
    };
    if formatted == src {
        return true;
    }
    if let Err(error) = fs::write(path, formatted) {
        eprintln!("{}: {error}", path.display());
        return false;
    }
    true
}

/// Reports `message` to stderr with the line and column of `offset` in `src`, the text of `path`.
fn report_at(path: &Path, src: &str, offset: usize, message: impl Display) {
    let (line, column) = line_column(src, offset);
    eprintln!("{}:{line}:{column}: {message}", path.display());
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
