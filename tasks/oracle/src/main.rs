//! Asks nu whether formatting keeps what it parses.
//! Each file must give the same AST and the same flattened tokens before and after formatting.
//! Positions, span ids, and the layout in the tokens are left aside.
//! The AST refers to the blocks inside by id only, so inside them only the tokens are compared.
//! Those show neither `;` nor line breaks between statements.
//! Nu parses both versions of a file in the directory of the file before formatting.
//!
//! `format` formats files with nuten and asks about each.
//! Before asking, it checks each `.nu` file for what `nuten_format::invariants` asks of any input.
//! It checks a file with syntax errors too before skipping it, for the invariants of the parser.
//! It asks nu too whether nuten reads each `.nuon` file into the value nu reads.
//! `tasks/oracle/test_data` holds files to run it on.
//! Its `nufmt` holds the cases nushell/nufmt broke, each headed by the URL of its report.
//! It takes files, and directories to search for `.nu` and `.nuon` files.
//! It skips a file that is too large, is not UTF-8, or has syntax errors, and says so.
//! It skips a `.nuon` file that nu reads and nuten leaves unread too.
//! It exits with 1 when any other file cannot be checked, or its AST or value differs.
//! It exits with 1 too when a `.nu` file breaks an invariant.
//! It exits with 2 when the arguments name no `.nu` or `.nuon` file.
//! It formats with the defaults, or with the config file `--config` names.
//! It exits with 2 too when that file cannot be loaded.
//!
//! `compare` asks about a file and the version of it that any formatter wrote.
//! It exits with 1 when either cannot be read, or their ASTs differ.
//!
//! Both exit with 2 when the arguments are invalid.
//! Both exit with 2 too when nu cannot be run, or is not of the series the formatter targets.

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

use nuten_format::{Config, format, invariants};
use usage::{Args, Cli, Subcommands};

#[derive(Cli)]
#[usage(bin = "nuten-oracle", unknown_flags = "error")]
struct Oracle {
    #[usage(subcommand)]
    command: Command,
}

#[derive(Subcommands)]
enum Command {
    /// Format files with nuten, check the invariants, and ask nu whether formatting keeps the AST
    Format(Format),
    /// Ask nu whether it parses a file the same as the version of it a formatter wrote
    Compare(Compare),
}

#[derive(Args)]
struct Format {
    /// Skip files larger than this many bytes
    #[usage(long, default = "524288")]
    max_size: u64,
    /// Config file to format with, instead of the defaults
    #[usage(long)]
    config: Option<PathBuf>,
    /// Files, and directories to search for `.nu` and `.nuon` files
    #[usage(required)]
    paths: Vec<PathBuf>,
}

#[derive(Args)]
struct Compare {
    /// The file before formatting
    before: PathBuf,
    /// The file after formatting
    after: PathBuf,
}

fn main() -> ExitCode {
    let oracle = Oracle::parse();
    if let Err(error) = nu::check_version() {
        eprintln!("{error}");
        return ExitCode::from(2);
    }
    match oracle.command {
        Command::Format(args) => run_format(&args),
        Command::Compare(args) => run_compare(&args),
    }
}

fn run_format(args: &Format) -> ExitCode {
    let config = match args.config.as_deref().map(nuten_config::load_given) {
        None => Config::default(),
        Some(Ok(config)) => config.format,
        Some(Err(problems)) => {
            eprintln!("{}", problems.join("\n"));
            return ExitCode::from(2);
        },
    };
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
        let checked = panic::catch_unwind(|| check(path, args.max_size, &config))
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

fn run_compare(args: &Compare) -> ExitCode {
    let read = |path: &Path| {
        fs::read_to_string(path).map_err(|error| format!("{}: {error}", path.display()))
    };
    let compared = read(&args.before).and_then(|before| {
        let after = read(&args.after)?;
        same_ast(&before, &after, dir_of(&args.before))
            .map_err(|problem| format!("{}: {problem}", args.before.display()))
    });
    match compared {
        Ok(()) => ExitCode::SUCCESS,
        Err(problem) => {
            println!("{problem}");
            ExitCode::FAILURE
        },
    }
}

/// `path` if it is not a directory, or else every `.nu` and `.nuon` file below it.
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
    /// Formatting keeps the invariants, and nu parses the same AST before and after it.
    /// Or nu reads the NUON as nuten does.
    Passed,
    /// Not checked, for the reason given.
    Skipped(String),
}

/// Checks one file; an error says in one line why it fails.
fn check(path: &Path, max_size: u64, config: &Config) -> Result<Checked, String> {
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
    if let Some(checked) = invariants_verdict(&invariants::check(&src, config))? {
        return Ok(checked);
    }
    let Ok(output) = format(&src, config) else {
        unreachable!("the source has no syntax errors")
    };
    same_ast(&src, &output, dir_of(path)).map(|()| Checked::Passed)
}

/// The outcome for a `.nu` file that the invariants decide, or `None` to ask nu.
fn invariants_verdict(checked: &invariants::Checked) -> Result<Option<Checked>, String> {
    if !checked.violations.is_empty() {
        let violations: Vec<_> = checked.violations.iter().map(ToString::to_string).collect();
        return Err(violations.join("; "));
    }
    Ok((!checked.syntax_errors.is_empty()).then(|| Checked::Skipped("syntax errors".to_string())))
}

/// The directory nu parses `path` in, where it finds the modules `path` uses by relative path.
fn dir_of(path: &Path) -> Option<&Path> {
    path.parent().filter(|dir| !dir.as_os_str().is_empty())
}

/// Fails, in one line, when nu parses `before` and `after` differently, or cannot parse them.
fn same_ast(before: &str, after: &str, dir: Option<&Path>) -> Result<(), String> {
    // Nu parses the same text into the same AST.
    if before == after {
        return Ok(());
    }
    let before = nu::ast(before, dir)?;
    let after = nu::ast(after, dir).map_err(|error| format!("after formatting, {error}"))?;
    match ast::difference(&before, &after)? {
        Some(difference) => Err(difference),
        None => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use nuten_format::{
        Config,
        invariants::{self, Violation},
    };

    use super::invariants_verdict;
    use crate::Checked;

    fn violation(detail: &str) -> Violation {
        Violation {
            invariant: "comment preservation",
            detail: detail.to_string(),
        }
    }

    #[test]
    fn file_keeping_invariants_is_left_to_nu() {
        let checked = invariants::check("ls | length\n", &Config::default());
        assert!(matches!(invariants_verdict(&checked), Ok(None)));
    }

    #[test]
    fn file_with_syntax_errors_is_skipped() {
        let checked = invariants::check("ls |", &Config::default());
        assert!(matches!(
            invariants_verdict(&checked),
            Ok(Some(Checked::Skipped(reason))) if reason == "syntax errors"
        ));
    }

    #[test]
    fn violations_fail_file_in_one_line() {
        let checked = invariants::Checked {
            violations: vec![violation("a"), violation("b")],
            ..invariants::check("ls\n", &Config::default())
        };
        assert_eq!(
            invariants_verdict(&checked).unwrap_err(),
            "comment preservation: a; comment preservation: b"
        );
    }

    #[test]
    fn violations_fail_file_even_with_syntax_errors() {
        let checked = invariants::Checked {
            violations: vec![violation("a")],
            ..invariants::check("ls |", &Config::default())
        };
        assert_eq!(
            invariants_verdict(&checked).unwrap_err(),
            "comment preservation: a"
        );
    }
}
