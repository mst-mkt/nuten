//! The command-line interface.
//!
//! `nuten format` exits with 0 when every file is formatted.
//! It exits with 2 when any file has syntax errors or cannot be read or written.
//! The other files are still formatted then.
//! Exit code 1 is kept for a future check mode.
//! Any command exits with 2 when the arguments are invalid.

#![expect(clippy::print_stderr, reason = "errors go to the user")]

use std::{
    fs,
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
    }
}

fn format_files(files: &[PathBuf]) -> ExitCode {
    let config = Config::default();
    // Not `any`: the files after a failure are still to be formatted.
    let failures = files
        .iter()
        .filter(|path| !format_file(path, &config))
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
                let (line, column) = line_column(&src, error.range.start().into());
                eprintln!("{}:{line}:{column}: {}", path.display(), error.kind);
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
