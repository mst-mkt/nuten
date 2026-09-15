//! Asks nu whether formatting keeps what it parses: each file must give the
//! same AST, positions aside, before and after formatting.
//!
//! Takes files, and directories to search for `.nu` files. Skips a file that
//! is too large, is not UTF-8, or has syntax errors, and says so. Exits with 1
//! when any other file cannot be checked or its AST differs, and with 2 when
//! the arguments are invalid or name no `.nu` file, or nu cannot be run or is
//! not of the series the formatter targets.

#![expect(
    clippy::print_stderr,
    clippy::print_stdout,
    reason = "reports go to the user"
)]

mod ast;
mod nu;

use std::{
    fs, io, panic,
    path::{Path, PathBuf},
    process::ExitCode,
};

use nuten_format::{Config, format};
use usage::Cli;

#[derive(Cli)]
#[usage(bin = "nuten_oracle", unknown_flags = "error")]
struct Args {
    /// Skip files larger than this many bytes
    #[usage(long, default = "524288")]
    max_size: u64,
    /// Files, and directories to search for `.nu` files
    #[usage(required)]
    paths: Vec<PathBuf>,
}

fn main() -> ExitCode {
    let args = Args::parse();
    if let Err(error) = nu::check_version() {
        eprintln!("{error}");
        return ExitCode::from(2);
    }
    let mut files = Vec::new();
    for path in &args.paths {
        nu_files(path, &mut files);
    }
    files.sort();
    files.dedup();
    if files.is_empty() {
        eprintln!("no .nu files in the paths given");
        return ExitCode::from(2);
    }
    let (mut failed, mut skipped) = (0, 0);
    for path in &files {
        // The panic hook has printed the message already.
        let checked = panic::catch_unwind(|| check(path, args.max_size))
            .unwrap_or_else(|_| Err("the formatter panics".to_string()));
        match checked {
            Ok(Checked::Kept) => {},
            Ok(Checked::Skipped(reason)) => {
                eprintln!("skipping {}: {reason}", path.display());
                skipped += 1;
            },
            Err(problem) => {
                println!("{}: {problem}", path.display());
                failed += 1;
            },
        }
    }
    eprintln!(
        "files: {}, failed: {failed}, skipped: {skipped}",
        files.len()
    );
    if failed == 0 {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

/// `path` if it is not a directory, or else every `.nu` file below it,
/// hidden directories aside.
fn nu_files(path: &Path, files: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(path).and_then(Iterator::collect::<io::Result<Vec<_>>>) else {
        // A file, or a directory that cannot be read, which `check` reports.
        files.push(path.to_path_buf());
        return;
    };
    for entry in entries {
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if entry.file_type().is_ok_and(|kind| kind.is_dir()) {
            if !name.starts_with('.') {
                nu_files(&entry.path(), files);
            }
        } else if name.ends_with(".nu") {
            files.push(entry.path());
        }
    }
}

/// A file whose check did not fail.
enum Checked {
    /// Nu parses the same AST before and after formatting.
    Kept,
    /// Not checked, for the reason given.
    Skipped(String),
}

/// Checks one file; an error says in one line why it fails.
fn check(path: &Path, max_size: u64) -> Result<Checked, String> {
    let size = fs::metadata(path).map_err(|error| error.to_string())?.len();
    if size > max_size {
        return Ok(Checked::Skipped(format!("larger than {max_size} bytes")));
    }
    let src = match fs::read_to_string(path) {
        Ok(src) => src,
        Err(error) if error.kind() == io::ErrorKind::InvalidData => {
            return Ok(Checked::Skipped(error.to_string()));
        },
        Err(error) => return Err(error.to_string()),
    };
    let Ok(output) = format(&src, &Config::default()) else {
        return Ok(Checked::Skipped("syntax errors".to_string()));
    };
    // Nu parses the same text into the same AST.
    if output == src {
        return Ok(Checked::Kept);
    }
    let before = nu::ast(&src)?;
    let after = nu::ast(&output).map_err(|error| format!("after formatting, {error}"))?;
    match ast::difference(&before, &after)? {
        Some(difference) => Err(difference),
        None => Ok(Checked::Kept),
    }
}
