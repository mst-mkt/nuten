use std::path::PathBuf;

use usage::{Args, Cli, Subcommands, ValueEnum};

use crate::problem::Problem;

#[derive(Cli)]
#[usage(bin = "nuten", version, unknown_flags = "error", completion)]
pub(crate) struct Nuten {
    #[usage(subcommand)]
    pub(crate) command: Command,
}

#[derive(Subcommands)]
pub(crate) enum Command {
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
pub(crate) struct Completions {
    /// Shell the script is for
    #[usage(value_enum)]
    pub(crate) shell: Shell,
}

#[derive(Clone, Copy, ValueEnum)]
#[expect(
    clippy::enum_variant_names,
    reason = "`PowerShell` is the shell's own name, not a kind of `Shell`"
)]
pub(crate) enum Shell {
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
pub(crate) struct Lsp {
    /// Serve on stdin and stdout, as by default; accepted for clients that pass it
    #[usage(long)]
    stdio: bool,
}

#[derive(Args)]
#[usage(args_override_self = false)]
pub(crate) struct Format {
    /// Config file for all files, instead of the one nearest each file
    #[usage(
        short = 'c',
        long,
        value_hint = usage::ValueHint::FilePath,
        extensions("nuon", "jsonc", "json")
    )]
    pub(crate) config: Option<PathBuf>,
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
    pub(crate) stdin_filepath: Option<PathBuf>,
    /// Files to format in place; a directory stands for the `.nu` and `.nuon` files under it;
    /// `-` for stdin
    #[usage(default = ".", value_hint = usage::ValueHint::FilePath, extensions("nu", "nuon"))]
    pub(crate) paths: Vec<PathBuf>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub(crate) enum Reporter {
    /// Annotations of GitHub Actions
    Github,
    /// Code Quality report of GitLab CI
    Gitlab,
}

const STDIN_PATH: &str = "-";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Mode {
    Write,
    Check,
    List,
    Report(Reporter),
}

pub(crate) fn mode(args: &Format) -> Mode {
    match (args.check, args.list_different, args.reporter) {
        (_, _, Some(reporter)) => Mode::Report(reporter),
        (true, _, _) => Mode::Check,
        (_, true, _) => Mode::List,
        _ => Mode::Write,
    }
}

pub(crate) fn reads_stdin(args: &Format) -> bool {
    args.paths.iter().any(|path| path.as_os_str() == STDIN_PATH)
}

/// Not `conflicts` of usage, which relates arguments and cannot tell `-` from other paths.
pub(crate) fn argument_problem(args: &Format) -> Option<Problem> {
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
