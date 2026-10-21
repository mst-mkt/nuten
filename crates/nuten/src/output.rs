use std::{
    io::{self, Write},
    path::{Path, PathBuf},
    process::ExitCode,
};

use crate::{
    cli::{Mode, Reporter},
    diff::Difference,
    problem::Problem,
    reporter::{github, gitlab},
    style::Style,
    view::view,
};

#[derive(Debug)]
pub(crate) struct Changed {
    pub(crate) path: PathBuf,
    /// Only for the modes that tell how a file changes.
    pub(crate) detail: Option<Detail>,
}

#[cfg(test)]
impl Changed {
    pub(crate) fn sample(name: &str) -> Changed {
        Changed {
            path: PathBuf::from(name),
            detail: Some(Detail {
                text: format!("{name}\n"),
                changed_lines: 3,
                whitespace_only_lines: 1,
            }),
        }
    }
}

#[derive(Debug)]
pub(crate) struct Detail {
    text: String,
    pub(crate) changed_lines: usize,
    pub(crate) whitespace_only_lines: usize,
}

pub(crate) fn detail(
    path: &Path,
    src: &str,
    formatted: &str,
    mode: Mode,
    style: Style,
) -> Option<Detail> {
    let text_of: fn(&Path, &Difference<'_>, Style) -> String = match mode {
        Mode::Write | Mode::List => return None,
        Mode::Check => view,
        Mode::Report(Reporter::Github) => {
            |path, difference, _| github::file_annotation(path, difference)
        },
        Mode::Report(Reporter::Gitlab) => {
            |path, difference, _| gitlab::file_issue(path, difference)
        },
    };
    let difference = Difference::new(src, formatted);
    Some(Detail {
        text: text_of(path, &difference, style),
        changed_lines: difference.changed_lines(),
        whitespace_only_lines: difference.whitespace_only_lines(),
    })
}

pub(crate) fn finish(
    changed: &[Changed],
    problems: Vec<Problem>,
    mode: Mode,
    style: Style,
) -> ExitCode {
    let written = write_stdout(&stdout_text(changed, &problems, mode, style));
    let problems: Vec<_> = problems.into_iter().chain(written.err()).collect();
    if !problems.is_empty() {
        fail(&problems)
    } else if mode != Mode::Write && !changed.is_empty() {
        ExitCode::from(1)
    } else {
        ExitCode::SUCCESS
    }
}

pub(crate) fn print(text: &str) -> ExitCode {
    match write_stdout(text) {
        Ok(()) => ExitCode::SUCCESS,
        Err(problem) => fail(&[problem]),
    }
}

pub(crate) fn fail(problems: &[Problem]) -> ExitCode {
    let error = Style::stderr().error("error:");
    for problem in problems {
        write_stderr(&format!("{error} {problem}"));
    }
    ExitCode::from(2)
}

/// Not a panic or a failure when stderr takes nothing, since the exit code still tells the outcome.
pub(crate) fn write_stderr(line: &str) {
    writeln!(io::stderr(), "{line}").ok();
}

fn write_stdout(text: &str) -> Result<(), Problem> {
    match io::stdout().write_all(text.as_bytes()) {
        // A reader that stops early, as `head` does, took what it wanted.
        Err(error) if error.kind() == io::ErrorKind::BrokenPipe => Ok(()),
        result => result.map_err(|error| Problem::new(format!("stdout: {error}"))),
    }
}

/// A reporter writes problems too, since what reads it reads only stdout.
fn stdout_text(changed: &[Changed], problems: &[Problem], mode: Mode, style: Style) -> String {
    match mode {
        Mode::Write => String::new(),
        Mode::Check => views_text(changed, style),
        Mode::List => listing(changed.iter().map(|changed| changed.path.as_path())),
        Mode::Report(Reporter::Github) => {
            detail_texts(changed).collect::<String>()
                + &problems
                    .iter()
                    .map(github::problem_annotation)
                    .collect::<String>()
        },
        Mode::Report(Reporter::Gitlab) => gitlab::report(
            detail_texts(changed)
                .map(str::to_string)
                .chain(problems.iter().filter_map(gitlab::problem_issue)),
        ),
    }
}

fn detail_texts(changed: &[Changed]) -> impl Iterator<Item = &str> {
    changed
        .iter()
        .filter_map(|changed| Some(changed.detail.as_ref()?.text.as_str()))
}

fn views_text(changed: &[Changed], style: Style) -> String {
    if changed.is_empty() {
        return String::new();
    }
    heading(changed.len(), style) + &detail_texts(changed).collect::<String>()
}

fn heading(count: usize, style: Style) -> String {
    let files = if count == 1 { "file" } else { "files" };
    style.bold(&format!("{count} {files} would be reformatted:")) + "\n\n"
}

fn listing<'a>(paths: impl IntoIterator<Item = &'a Path>) -> String {
    // Lossy for names that are not UTF-8, as the lists of other formatters are.
    paths
        .into_iter()
        .map(|path| path.display().to_string() + "\n")
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{Changed, views_text};
    use crate::style::Style;

    #[test]
    fn views_text_heads_views_with_count_in_bold() {
        assert_eq!(
            views_text(&[Changed::sample("a")], Style::COLORED),
            "\u{1b}[1m1 file would be reformatted:\u{1b}[0m\n\na\n"
        );
        assert_eq!(
            views_text(&[Changed::sample("a"), Changed::sample("b")], Style::PLAIN),
            "2 files would be reformatted:\n\na\nb\n"
        );
        assert_eq!(views_text(&[], Style::PLAIN), "");
    }
}
