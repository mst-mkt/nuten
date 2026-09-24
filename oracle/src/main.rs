//! Asks nu whether formatting keeps what it parses.
//! Each file must give the same AST and the same flattened tokens before and after formatting.
//! Positions, span ids, line endings, and the layout in the tokens are left aside.
//! The AST refers to the blocks inside by id only, so inside them only the tokens are compared.
//! Those show neither `;` nor line breaks between statements.
//! Nu parses each file in its directory.
//! Asks nu too whether nuten reads each `.nuon` file into the value nu reads.
//! `oracle/test_data` holds NUON files to run the oracle on.
//!
//! Takes files, and directories to search for `.nu` and `.nuon` files.
//! Skips a file that is too large, is not UTF-8, or has syntax errors, and says so.
//! Skips a `.nuon` file that nu reads and nuten leaves unread too.
//! Exits with 1 when any other file cannot be checked, or its AST or value differs.
//! Exits with 2 when the arguments are invalid or name no `.nu` or `.nuon` file.
//! Exits with 2 too when nu cannot be run, or is not of the series the formatter targets.

#![expect(
    clippy::print_stderr,
    clippy::print_stdout,
    reason = "reports go to the user"
)]

mod ast;
mod nu;
mod nuon;

use std::{
    fs, io, panic,
    path::{Path, PathBuf},
    process::ExitCode,
};

use nuten_format::{Config, format};
use usage::Cli;

#[derive(Cli)]
#[usage(bin = "nuten-oracle", unknown_flags = "error")]
struct Args {
    /// Skip files larger than this many bytes
    #[usage(long, default = "524288")]
    max_size: u64,
    /// Files, and directories to search for `.nu` and `.nuon` files
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
        files_to_check(path, &mut files);
    }
    files.sort();
    files.dedup();
    if files.is_empty() {
        eprintln!("no .nu or .nuon files in the paths given");
        return ExitCode::from(2);
    }
    let (mut failed, mut skipped) = (0, 0);
    for path in &files {
        // The panic hook has printed the message already.
        let checked = panic::catch_unwind(|| check(path, args.max_size))
            .unwrap_or_else(|_| Err("nuten panics".to_string()));
        match checked {
            Ok(Checked::Passed) => {},
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

/// `path` if it is not a directory, or else every `.nu` and `.nuon` file below it.
/// Hidden directories are left aside.
fn files_to_check(path: &Path, files: &mut Vec<PathBuf>) {
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
                files_to_check(&entry.path(), files);
            }
        } else if entry
            .path()
            .extension()
            .is_some_and(|extension| extension == "nu" || extension == "nuon")
        {
            files.push(entry.path());
        }
    }
}

/// A file whose check did not fail.
#[derive(Debug)]
enum Checked {
    /// Nu parses the same AST before and after formatting, or reads the NUON as nuten does.
    Passed,
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
    if path
        .extension()
        .is_some_and(|extension| extension == "nuon")
    {
        return nuon::check(&src);
    }
    let Ok(output) = format(&src, &Config::default()) else {
        return Ok(Checked::Skipped("syntax errors".to_string()));
    };
    // Nu parses the same text into the same AST.
    if output == src {
        return Ok(Checked::Passed);
    }
    let dir = path.parent().filter(|dir| !dir.as_os_str().is_empty());
    let before = nu::ast(&src, dir)?;
    let after = nu::ast(&output, dir).map_err(|error| format!("after formatting, {error}"))?;
    match ast::difference(&before, &after)? {
        Some(difference) => Err(difference),
        None => Ok(Checked::Passed),
    }
}
