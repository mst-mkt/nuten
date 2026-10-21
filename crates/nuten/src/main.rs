mod cli;
mod diff;
mod files;
mod gitignore;
mod output;
mod overwrite;
mod parallel;
mod picture;
mod problem;
mod reporter;
mod run;
mod style;
mod summary;
mod view;

use std::process::ExitCode;

use nuten_lsp::Exit;

use crate::{
    cli::{Command, Nuten},
    output::{fail, print},
    problem::Problem,
};

fn main() -> ExitCode {
    match Nuten::parse().command {
        Command::Format(args) => run::format(&args),
        Command::Schema => print(&nuten_config::schema()),
        Command::Lsp(_) => serve_lsp(),
        Command::Completions(args) => print(&Nuten::completion_script(args.shell.into())),
    }
}

/// Exits as the protocol asks.
fn serve_lsp() -> ExitCode {
    match nuten_lsp::serve_stdio() {
        Ok(Exit::AfterShutdown) => ExitCode::SUCCESS,
        Ok(Exit::WithoutShutdown) => ExitCode::from(1),
        Err(problem) => fail(&[Problem::new(problem)]),
    }
}
