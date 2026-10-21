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

/// Stdout, written as each file is done.
#[derive(Debug)]
pub(crate) struct Output<W> {
    writer: W,
    mode: Mode,
    changed: usize,
    failure: Option<Problem>,
}

impl<W: Write> Output<W> {
    pub(crate) fn new(mode: Mode, writer: W) -> Self {
        Self {
            writer,
            mode,
            changed: 0,
            failure: None,
        }
    }

    pub(crate) fn add(&mut self, changed: &Changed) {
        let detail = changed.detail.as_ref().map_or("", |detail| &detail.text);
        match self.mode {
            Mode::Write => {},
            // Lossy for names that are not UTF-8, as the lists of other formatters are.
            Mode::List => self.write(&format!("{}\n", changed.path.display())),
            Mode::Check | Mode::Report(Reporter::Github) => self.write(detail),
            Mode::Report(Reporter::Gitlab) => self.write(&gitlab::entry(self.changed, detail)),
        }
        self.changed += 1;
    }

    /// A reporter writes problems too, since what reads it reads only stdout.
    fn end(&mut self, problems: &[Problem]) {
        match self.mode {
            Mode::Write | Mode::Check | Mode::List => {},
            Mode::Report(Reporter::Github) => {
                let annotations: String = problems.iter().map(github::problem_annotation).collect();
                self.write(&annotations);
            },
            Mode::Report(Reporter::Gitlab) => {
                let mut count = self.changed;
                for issue in problems.iter().filter_map(gitlab::problem_issue) {
                    self.write(&gitlab::entry(count, &issue));
                    count += 1;
                }
                self.write(gitlab::end(count));
            },
        }
    }

    fn write(&mut self, text: &str) {
        if self.failure.is_none() {
            self.failure = write_stdout(&mut self.writer, text).err();
        }
    }

    #[cfg(test)]
    pub(crate) fn writer(&self) -> &W {
        &self.writer
    }
}

pub(crate) fn finish(mut output: Output<impl Write>, problems: Vec<Problem>) -> ExitCode {
    output.end(&problems);
    let problems: Vec<_> = problems.into_iter().chain(output.failure).collect();
    if !problems.is_empty() {
        fail(&problems)
    } else if output.mode != Mode::Write && output.changed > 0 {
        ExitCode::from(1)
    } else {
        ExitCode::SUCCESS
    }
}

pub(crate) fn print(text: &str) -> ExitCode {
    match write_stdout(&mut io::stdout(), text) {
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

/// Flushes, so that the text shows at once and a failure is reported.
fn write_stdout(stdout: &mut impl Write, text: &str) -> Result<(), Problem> {
    match stdout
        .write_all(text.as_bytes())
        .and_then(|()| stdout.flush())
    {
        // A reader that stops early, as `head` does, took what it wanted.
        Err(error) if error.kind() == io::ErrorKind::BrokenPipe => Ok(()),
        result => result.map_err(|error| Problem::new(format!("stdout: {error}"))),
    }
}

#[cfg(test)]
mod tests {
    use std::io::{self, Write};

    use super::{Changed, Output, Problem};
    use crate::cli::Mode;

    #[derive(Debug, Default)]
    struct Full {
        writes: usize,
    }

    impl Write for Full {
        fn write(&mut self, _: &[u8]) -> io::Result<usize> {
            self.writes += 1;
            Err(io::Error::other("e"))
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn output_writes_nothing_after_failure() {
        let mut output = Output::new(Mode::Check, Full::default());
        output.add(&Changed::sample("a"));
        output.add(&Changed::sample("b"));
        output.end(&[]);
        assert_eq!(output.writer.writes, 1);
        assert_eq!(output.failure, Some(Problem::new("stdout: e")));
    }
}
