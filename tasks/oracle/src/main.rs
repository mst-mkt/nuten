//! The AST refers to the blocks inside by id only, so inside them only the tokens are compared.
//! Those show neither `;` nor line breaks between statements.

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

use nuten_format::{Config, invariants};
use usage::{Args, Cli, Subcommands};

#[derive(Cli)]
#[usage(bin = "nuten-oracle", unknown_flags = "error")]
struct Oracle {
    #[usage(subcommand)]
    command: Command,
}

#[derive(Subcommands)]
enum Command {
    /// Format files, check the invariants, and ask nu whether formatting keeps what it parses
    Format(Format),
    /// Ask nu whether it parses a file the same as the version of it a formatter wrote
    Compare(Compare),
}

#[derive(Args)]
struct Format {
    /// Skip files larger than this many bytes
    #[usage(long, default = "524288")]
    max_size: u64,
    /// Config file to format with, and to make the variants from, instead of the defaults
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

#[derive(Debug, PartialEq, Eq)]
enum Checked {
    Passed,
    Skipped(String),
}

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
    let outputs = formatted(&src, config)?;
    if path
        .extension()
        .is_some_and(|extension| extension == "nuon")
    {
        return nuon::check(&src, outputs.as_deref(), dir_of(path));
    }
    let Some(outputs) = outputs else {
        return rejection_verdict(nu::parse_error(&src, dir_of(path))?);
    };
    same_asts(&src, &outputs, dir_of(path)).map(|()| Checked::Passed)
}

/// Nu rejects the file too, whatever `nu_error` is, so nothing tells whether nuten is wrong.
fn rejection_verdict(nu_error: Option<String>) -> Result<Checked, String> {
    match nu_error {
        Some(kind) => Ok(Checked::Skipped(format!(
            "syntax errors, and nu reports {kind}"
        ))),
        None => Err("nuten reports syntax errors, though nu parses it".to_string()),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Output {
    text: String,
    variant: Option<&'static str>,
}

fn labeled(variant: Option<&str>, problem: String) -> String {
    match variant {
        Some(name) => format!("with {name}: {problem}"),
        None => problem,
    }
}

/// Asking nu costs a process each time, so every config is checked for the invariants first.
fn formatted(src: &str, config: &Config) -> Result<Option<Vec<Output>>, String> {
    let configs = [(None, config.clone())].into_iter().chain(
        invariants::VARIANTS
            .iter()
            .map(|variant| (Some(variant.name), variant.of(config))),
    );
    let outputs = configs
        .map(|(variant, config)| {
            // Catching here names the variant; the panic hook has printed the message already.
            let checked = panic::catch_unwind(|| invariants::check(src, &config))
                .map_err(|_| labeled(variant, "nuten panics".to_string()))?;
            kept_output(checked, variant)
        })
        .collect::<Result<Vec<_>, String>>()?;
    if outputs.iter().all(Option::is_none) {
        return Ok(None);
    }
    let outputs = outputs
        .into_iter()
        .collect::<Option<Vec<_>>>()
        .ok_or("nuten gives output with some configs only")?;
    Ok(Some(distinct(src, &outputs)))
}

fn kept_output(
    checked: invariants::Checked,
    variant: Option<&'static str>,
) -> Result<Option<Output>, String> {
    invariants_kept(&checked).map_err(|problem| labeled(variant, problem))?;
    Ok(checked.output.map(|text| Output {
        text,
        variant,
    }))
}

/// Nu parses the same text into the same AST, so only the first of each text needs asking about.
fn distinct(src: &str, outputs: &[Output]) -> Vec<Output> {
    outputs
        .iter()
        .enumerate()
        .filter(|&(index, output)| {
            output.text != src
                && outputs[..index]
                    .iter()
                    .all(|earlier| earlier.text != output.text)
        })
        .map(|(_, output)| output.clone())
        .collect()
}

fn invariants_kept(checked: &invariants::Checked) -> Result<(), String> {
    if checked.violations.is_empty() {
        return Ok(());
    }
    let violations: Vec<_> = checked.violations.iter().map(ToString::to_string).collect();
    Err(violations.join("; "))
}

/// Nu parses `path` in its directory, so that it finds the modules `path` uses by relative path.
fn dir_of(path: &Path) -> Option<&Path> {
    path.parent().filter(|dir| !dir.as_os_str().is_empty())
}

fn same_ast(before: &str, after: &str, dir: Option<&Path>) -> Result<(), String> {
    // Nu parses the same text into the same AST.
    if before == after {
        return Ok(());
    }
    let after = Output {
        text: after.to_string(),
        variant: None,
    };
    same_asts(before, &[after], dir)
}

/// Asking nu costs a process each time, so nu parses `before` once, and not at all without outputs.
fn same_asts(before: &str, outputs: &[Output], dir: Option<&Path>) -> Result<(), String> {
    if outputs.is_empty() {
        return Ok(());
    }
    let before = nu::ast(before, dir)?;
    outputs.iter().try_for_each(|output| {
        nu::ast(&output.text, dir)
            .map_err(|error| format!("after formatting, {error}"))
            .and_then(|after| match ast::difference(&before, &after)? {
                Some(difference) => Err(difference),
                None => Ok(()),
            })
            .map_err(|problem| labeled(output.variant, problem))
    })
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use nuten_format::{
        Config, format,
        invariants::{self, Violation},
    };

    use super::{
        Checked, Output, distinct, formatted, invariants_kept, kept_output, labeled,
        rejection_verdict, same_ast, same_asts,
    };

    fn violation(detail: &str) -> Violation {
        Violation {
            invariant: "comment preservation",
            detail: detail.to_string(),
        }
    }

    fn output(text: &str, variant: Option<&'static str>) -> Output {
        Output {
            text: text.to_string(),
            variant,
        }
    }

    /// Nu cannot start in a directory that does not exist, so asking it there fails.
    fn nowhere() -> &'static Path {
        Path::new("/nonexistent")
    }

    #[test]
    fn file_keeping_invariants_gives_outputs_of_config_given_and_of_its_variants() {
        let given = Config {
            indent_width: 2,
            ..Config::default()
        };
        let src = "[a,b]\n";
        let variants = invariants::VARIANTS
            .iter()
            .map(|variant| (Some(variant.name), variant.of(&given)));
        let outputs: Vec<_> = [(None, given.clone())]
            .into_iter()
            .chain(variants)
            .map(|(variant, config)| output(&format(src, &config).unwrap(), variant))
            .collect();
        assert_eq!(formatted(src, &given), Ok(Some(distinct(src, &outputs))));
    }

    #[test]
    fn outputs_same_as_source_or_earlier_one_are_dropped() {
        let outputs = [
            output("a", None),
            output("b", Some("x")),
            output("b", Some("y")),
            output("c", Some("z")),
        ];
        assert_eq!(
            distinct("a", &outputs),
            [output("b", Some("x")), output("c", Some("z"))]
        );
    }

    #[test]
    fn problem_with_variant_names_it() {
        assert_eq!(labeled(Some("v"), "p".to_string()), "with v: p");
        assert_eq!(labeled(None, "p".to_string()), "p");
    }

    #[test]
    fn violations_with_variant_fail_naming_it() {
        let checked = invariants::Checked {
            violations: vec![violation("a")],
            ..invariants::check("ls\n", &Config::default())
        };
        assert_eq!(
            kept_output(checked, Some("v")).unwrap_err(),
            "with v: comment preservation: a"
        );
    }

    #[test]
    fn no_outputs_leave_nu_unasked() {
        assert_eq!(same_asts("ls", &[], Some(nowhere())), Ok(()));
    }

    #[test]
    fn same_texts_leave_nu_unasked() {
        assert_eq!(same_ast("(", "(", Some(nowhere())), Ok(()));
    }

    #[test]
    fn file_with_syntax_errors_gives_no_output() {
        assert_eq!(formatted("ls |", &Config::default()), Ok(None));
    }

    #[test]
    fn file_nu_rejects_too_is_skipped_naming_nu_error() {
        assert_eq!(
            rejection_verdict(Some("UnexpectedEof".to_string())),
            Ok(Checked::Skipped(
                "syntax errors, and nu reports UnexpectedEof".to_string()
            ))
        );
    }

    #[test]
    fn file_nu_parses_fails() {
        assert_eq!(
            rejection_verdict(None),
            Err("nuten reports syntax errors, though nu parses it".to_string())
        );
    }

    #[test]
    fn violations_fail_in_one_line() {
        let checked = invariants::Checked {
            violations: vec![violation("a"), violation("b")],
            ..invariants::check("ls\n", &Config::default())
        };
        assert_eq!(
            invariants_kept(&checked).unwrap_err(),
            "comment preservation: a; comment preservation: b"
        );
    }

    #[test]
    fn violations_fail_even_with_syntax_errors() {
        let checked = invariants::Checked {
            violations: vec![violation("a")],
            ..invariants::check("ls |", &Config::default())
        };
        assert_eq!(
            invariants_kept(&checked).unwrap_err(),
            "comment preservation: a"
        );
    }
}
