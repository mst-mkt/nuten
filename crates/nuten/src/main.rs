//! The command-line interface.
//!
//! Exits with 0 when every file is formatted, and with 2 when the arguments are invalid or any file has syntax errors or cannot be read or written; the other files are still formatted.
//! Exit code 1 is kept for a future check mode.

#![expect(clippy::print_stderr, reason = "errors go to the user")]

use std::{
    fs,
    path::{Path, PathBuf},
    process::ExitCode,
};

use nuten_format::{Config, format};
use usage::Cli;

#[derive(Cli)]
#[usage(bin = "nuten", version, unknown_flags = "error")]
struct Args {
    /// Files to format in place
    #[usage(required)]
    files: Vec<PathBuf>,
}

fn main() -> ExitCode {
    let args = Args::parse();
    let config = Config::default();
    // Not `any`: the files after a failure are still to be formatted.
    let failures = args
        .files
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
    fn line_and_column_start_at_one_and_count_characters() {
        assert_eq!(line_column("ls |", 3), (1, 4));
        assert_eq!(line_column("ls\n\nls |", 7), (3, 4));
        assert_eq!(line_column("ls\r\nls |", 7), (2, 4));
        assert_eq!(line_column("echo 日本 |", 12), (1, 9));
        assert_eq!(line_column("", 0), (1, 1));
    }
}
