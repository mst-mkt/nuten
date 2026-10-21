use std::{
    collections::{HashMap, HashSet},
    fs,
    hash::Hash,
    io::{self, Write},
    panic::{self, RefUnwindSafe, UnwindSafe},
    path::{Path, PathBuf},
    process::ExitCode,
};

use nuten_config::{Config, FormatConfig, Lookup, LookupError};
use nuten_line_index::located;

use crate::{
    cli::{self, Format, Mode},
    files::{self, Candidate, Entry},
    output::{self, Changed, Output, finish, print, write_stderr},
    overwrite::{Contents, Failure, OnDisk, overwrite},
    parallel,
    problem::Problem,
    style::Style,
    summary::{Counts, summary},
};

const STDIN_NAME: &str = "<stdin>";

#[derive(Debug)]
struct Report<W> {
    output: Output<W>,
    problems: Vec<Problem>,
    counts: Counts,
}

pub(crate) fn format(args: &Format) -> ExitCode {
    let mode = cli::mode(args);
    if let Some(problem) = cli::argument_problem(args) {
        return finish(Output::new(mode, io::stdout()), vec![problem]);
    }
    if cli::reads_stdin(args) {
        let filepath = args.stdin_filepath.as_deref();
        return format_stdin(filepath, args.config.as_deref(), mode);
    }
    match args
        .config
        .as_deref()
        .map(nuten_config::load_given)
        .transpose()
    {
        Ok(given) => format_paths(&args.paths, given.as_ref(), mode),
        Err(errors) => finish(Output::new(mode, io::stdout()), problems_of(errors)),
    }
}

fn format_stdin(filepath: Option<&Path>, config_file: Option<&Path>, mode: Mode) -> ExitCode {
    let name = filepath.unwrap_or(Path::new(STDIN_NAME));
    // Read in full first, so that a writer is not cut off when a config file has a problem.
    let src = io::read_to_string(io::stdin()).map_err(|error| vec![Problem::in_file(name, error)]);
    let formatted = src.and_then(|src| {
        let given = config_file
            .map(nuten_config::load_given)
            .transpose()
            .map_err(problems_of)?;
        let config = nuten_config::of_text(filepath, given.as_ref()).map_err(problems_of)?;
        let format = config
            .format_of_text(filepath)
            .map_err(|error| vec![error.into()])?;
        let formatted = match format {
            Some(format) => catch_panic(name, || format_text(name, &src, &format))??,
            // Written back as it is, since the caller takes stdout for what the file is to hold.
            None => src.clone(),
        };
        Ok((src, formatted))
    });
    let style = Style::stdout();
    let mut output = Output::new(mode, io::stdout());
    let problems = match formatted {
        Ok((_, formatted)) if mode == Mode::Write => return print(&formatted),
        Ok((src, formatted)) => {
            if formatted != src {
                output.add(&Changed {
                    path: name.to_path_buf(),
                    detail: output::detail(name, &src, &formatted, mode, style),
                });
            }
            Vec::new()
        },
        Err(problems) => problems,
    };
    finish(output, problems)
}

fn format_paths(paths: &[PathBuf], given: Option<&Config>, mode: Mode) -> ExitCode {
    let style = Style::stdout();
    let report = report(paths, given, mode, style);
    let problems = unique(report.problems.iter()).cloned().collect();
    let code = finish(report.output, problems);
    if let Some(summary) = summary(&report.counts, mode, Style::stderr()) {
        write_stderr(&summary);
    }
    code
}

fn report(
    paths: &[PathBuf],
    given: Option<&Config>,
    mode: Mode,
    style: Style,
) -> Report<io::Stdout> {
    let dirs = files::dirs(paths);
    let dir_lookups: HashMap<_, _> = dirs
        .iter()
        .copied()
        .zip(nuten_config::of_dirs(&dirs, given))
        .collect();
    let entries = files::expand(paths, &dir_lookups);
    let canonicals: Vec<_> = entries
        .iter()
        .filter_map(|entry| {
            let candidate = entry.as_ref().ok()?;
            Some(candidate.canonical.as_deref())
        })
        .collect();
    let lookups = nuten_config::of_files(&canonicals, given);
    let (taken, excluded) = taken(with_lookups(entries, lookups));
    // After the files are picked, since telling two files apart reads each.
    // Also keeps two threads off one file, which could read it halfway written.
    let jobs = files::distinct(taken, &excluded, |(candidate, _)| &candidate.path);
    // Not one per path, since a file given under two names is excluded once.
    let excluded_count =
        files::distinct(excluded.into_iter().map(Ok).collect(), &[], |path| path).len();
    let formats = !dir_lookups
        .values()
        .chain(jobs.iter().flatten().map(|(_, lookup)| lookup))
        .any(Lookup::is_wrong);
    let mut report = Report::new(Output::new(mode, io::stdout()), excluded_count);
    let mode = formats.then_some(mode);
    format_all(
        &jobs,
        |candidate, lookup| outcome(candidate, lookup, mode, style),
        |outcome| report.add(outcome),
    );
    report
}

type Job = Result<(Candidate, Lookup), Problem>;

/// `lookups` holds the config of each file of `entries`.
fn with_lookups(entries: Vec<Entry>, lookups: Vec<Lookup>) -> Vec<Job> {
    let mut lookups = lookups.into_iter();
    entries
        .into_iter()
        .map(|entry| {
            entry.map(|candidate| {
                let lookup = lookups.next().expect("a lookup for each file");
                (candidate, lookup)
            })
        })
        .collect()
}

fn taken(jobs: Vec<Job>) -> (Vec<Job>, Vec<PathBuf>) {
    let (taken, left): (Vec<_>, Vec<_>) = jobs
        .into_iter()
        .map(|job| {
            let pick = job.as_ref().map_or(Pick::Take, pick);
            (job, pick)
        })
        .partition(|(_, pick)| *pick == Pick::Take);
    let excluded = left
        .into_iter()
        .filter(|(_, pick)| *pick == Pick::Exclude)
        .filter_map(|(job, _)| Some(job.ok()?.0.path))
        .collect();
    (taken.into_iter().map(|(job, _)| job).collect(), excluded)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Pick {
    Take,
    Exclude,
    Skip,
}

/// A config with problems takes the file, so that they are reported.
fn pick((candidate, lookup): &(Candidate, Lookup)) -> Pick {
    let Lookup::Found(config) = lookup else {
        return Pick::Take;
    };
    // A file that cannot be resolved is taken, so that reading it reports why.
    let Ok(file) = &candidate.canonical else {
        return Pick::Take;
    };
    if !candidate.given && !config.includes(file) {
        Pick::Skip
    } else if config.excludes(file) {
        Pick::Exclude
    } else {
        Pick::Take
    }
}

#[derive(Debug)]
enum Outcome {
    Changed(Changed),
    Unchanged,
    /// Left alone, since some config file is wrong.
    Skipped(Vec<Problem>),
    Failed(Vec<Problem>),
    /// Of no file, such as a line of an ignore file.
    Unattached(Vec<Problem>),
}

fn format_all(
    jobs: &[Job],
    outcome_of: impl Fn(&Candidate, &Lookup) -> Outcome + Sync + RefUnwindSafe,
    each: impl FnMut(Outcome),
) {
    let outcome = |job: &Job| match job {
        Ok((candidate, lookup)) => catch_panic(&candidate.path, || outcome_of(candidate, lookup))
            .unwrap_or_else(Outcome::Failed),
        Err(problem) => Outcome::Unattached(vec![problem.clone()]),
    };
    parallel::map(jobs, outcome, each);
}

/// Takes `style`, since the detail of each file builds in parallel.
fn outcome(candidate: &Candidate, lookup: &Lookup, mode: Option<Mode>, style: Style) -> Outcome {
    match (lookup, mode) {
        (Lookup::Found(config), Some(mode)) => {
            let format = match &candidate.canonical {
                Ok(file) => config.format_of(file),
                // Reading the file reports why it cannot be resolved.
                Err(_) => config.format_without_file(),
            };
            match format_file(&candidate.path, &format, mode, style) {
                Ok(Some(changed)) => Outcome::Changed(changed),
                Ok(None) => Outcome::Unchanged,
                Err(problems) => Outcome::Failed(problems),
            }
        },
        (Lookup::Found(_), None) => Outcome::Skipped(Vec::new()),
        (Lookup::Unresolved(error), _) => Outcome::Failed(vec![error.clone().into()]),
        (Lookup::Wrong(errors), _) => Outcome::Skipped(problems_of(errors.clone())),
    }
}

impl<W: Write> Report<W> {
    fn new(output: Output<W>, excluded: usize) -> Self {
        Self {
            output,
            problems: Vec::new(),
            counts: Counts {
                excluded,
                ..Counts::default()
            },
        }
    }

    fn add(&mut self, outcome: Outcome) {
        let counts = &mut self.counts;
        match outcome {
            Outcome::Changed(changed) => {
                counts.changed += 1;
                if let Some(detail) = &changed.detail {
                    counts.changed_lines += detail.changed_lines;
                    counts.whitespace_only_lines += detail.whitespace_only_lines;
                }
                self.output.add(&changed);
            },
            Outcome::Unchanged => counts.unchanged += 1,
            Outcome::Skipped(problems) => {
                counts.skipped += 1;
                self.problems.extend(problems);
            },
            Outcome::Failed(problems) => {
                counts.failed += 1;
                self.problems.extend(problems);
            },
            Outcome::Unattached(problems) => self.problems.extend(problems),
        }
    }
}

fn format_file(
    path: &Path,
    config: &FormatConfig,
    mode: Mode,
    style: Style,
) -> Result<Option<Changed>, Vec<Problem>> {
    let io_problem = |error: io::Error| vec![Problem::in_file(path, error)];
    let src = read_regular_file(path).map_err(io_problem)?;
    let formatted = format_text(path, &src, config)?;
    if formatted == src {
        return Ok(None);
    }
    if mode == Mode::Write {
        overwrite(&OnDisk(path), src.as_bytes(), formatted.as_bytes())
            .map_err(|failure| vec![overwrite_problem(path, &failure)])?;
    }
    Ok(Some(Changed {
        path: path.to_path_buf(),
        detail: output::detail(path, &src, &formatted, mode, style),
    }))
}

/// Not `fs::read_to_string` alone, which waits forever on a FIFO.
fn read_regular_file(path: &Path) -> io::Result<String> {
    if fs::metadata(path)?.is_file() {
        fs::read_to_string(path)
    } else {
        Err(io::Error::other("not a regular file"))
    }
}

fn overwrite_problem(path: &Path, failure: &Failure) -> Problem {
    let error = &failure.error;
    match (failure.contents, failure.backup.as_deref()) {
        (Contents::New, Some(backup)) => Problem::in_file(
            backup,
            format!(
                "{error}; {} is formatted, but this backup of it is left behind",
                path.display()
            ),
        ),
        (Contents::Broken, Some(backup)) => Problem::in_file(
            path,
            format!("{error}; its old contents are in {}", backup.display()),
        ),
        (Contents::Broken, None) => {
            Problem::in_file(path, format!("{error}; it may be left partly written"))
        },
        (Contents::Old, Some(backup)) => Problem::in_file(
            path,
            format!("{error}; {} is left behind", backup.display()),
        ),
        (_, None) => Problem::in_file(path, error),
    }
}

fn format_text(path: &Path, src: &str, config: &FormatConfig) -> Result<String, Vec<Problem>> {
    nuten_format::format(src, config).map_err(|errors| {
        located(
            src,
            errors
                .errors
                .iter()
                .map(|error| (error.range.start().into(), error.kind)),
        )
        .into_iter()
        .map(|(line_column, kind)| Problem::at(path, line_column.into(), kind))
        .collect()
    })
}

/// Not left to end the run, which would lose the report of the other files.
fn catch_panic<T>(path: &Path, f: impl FnOnce() -> T + UnwindSafe) -> Result<T, Vec<Problem>> {
    panic::catch_unwind(f)
        .map_err(|_| vec![Problem::in_file(path, "nuten panicked while formatting")])
}

fn problems_of(errors: Vec<LookupError>) -> Vec<Problem> {
    errors.into_iter().map(Problem::from).collect()
}

fn unique<T: Copy + Eq + Hash>(items: impl Iterator<Item = T>) -> impl Iterator<Item = T> {
    let mut seen = HashSet::new();
    items.filter(move |item| seen.insert(*item))
}

#[cfg(test)]
mod tests {
    use std::{
        io::{self, Write},
        path::{Path, PathBuf},
    };

    use nuten_config::{FormatConfig, Lookup};

    use super::{
        Outcome, Problem, Report, catch_panic, format_all, format_text, overwrite_problem,
    };
    use crate::{
        cli::{Mode, Reporter},
        files::Candidate,
        output::{Changed, Output},
        overwrite::{Contents, Failure},
        problem::Position,
        summary::Counts,
    };

    #[test]
    fn format_text_places_each_syntax_error_at_its_line_and_column() {
        let problems =
            format_text(Path::new("a.nu"), "ls\nls ||", &FormatConfig::default()).unwrap_err();
        let places: Vec<_> = problems
            .into_iter()
            .map(|problem| (problem.path, problem.position))
            .collect();
        let position = Position {
            line: 2,
            column: Some(4),
        };
        assert_eq!(places, [(Some(PathBuf::from("a.nu")), Some(position))]);
    }

    #[test]
    fn catch_panic_turns_panic_into_problem_naming_file() {
        assert_eq!(
            catch_panic(Path::new("dir/a.nu"), || panic!("bug")),
            Err::<(), _>(vec![Problem::in_file(
                Path::new("dir/a.nu"),
                "nuten panicked while formatting"
            )])
        );
    }

    #[test]
    fn catch_panic_passes_on_value_of_f() {
        assert_eq!(catch_panic(Path::new("dir/a.nu"), || 1), Ok(1));
    }

    #[test]
    fn format_all_fails_only_file_that_panics() {
        let job = |path: &str| {
            let candidate = Candidate {
                path: PathBuf::from(path),
                canonical: Ok(PathBuf::from(path)),
                given: true,
            };
            Ok((candidate, Lookup::Wrong(Vec::new())))
        };
        let jobs = [job("a.nu"), job("b.nu"), job("c.nu")];
        let mut outcomes = Vec::new();
        format_all(
            &jobs,
            |candidate, _| {
                assert_ne!(candidate.path, Path::new("b.nu"), "bug");
                Outcome::Unchanged
            },
            |outcome| outcomes.push(outcome),
        );
        let [
            Outcome::Unchanged,
            Outcome::Failed(problems),
            Outcome::Unchanged,
        ] = outcomes.as_slice()
        else {
            panic!("{outcomes:?}");
        };
        assert_eq!(
            *problems,
            [Problem::in_file(
                Path::new("b.nu"),
                "nuten panicked while formatting"
            )]
        );
    }

    #[test]
    fn overwrite_problem_says_what_file_and_backup_hold() {
        let cases = [
            (Contents::Old, None, "a.nu: e"),
            (Contents::Old, Some("b"), "a.nu: e; b is left behind"),
            (
                Contents::New,
                Some("b"),
                "b: e; a.nu is formatted, but this backup of it is left behind",
            ),
            (
                Contents::Broken,
                Some("b"),
                "a.nu: e; its old contents are in b",
            ),
            (
                Contents::Broken,
                None,
                "a.nu: e; it may be left partly written",
            ),
        ];
        for (contents, backup, expected) in cases {
            let failure = Failure {
                error: io::Error::other("e"),
                contents,
                backup: backup.map(PathBuf::from),
            };
            assert_eq!(
                overwrite_problem(Path::new("a.nu"), &failure).to_string(),
                expected
            );
        }
    }

    #[derive(Debug, Default)]
    struct Flushed {
        written: Vec<u8>,
        flushed: Vec<u8>,
    }

    impl Write for Flushed {
        fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            self.written.extend_from_slice(buf);
            Ok(buf.len())
        }

        fn flush(&mut self) -> io::Result<()> {
            self.flushed.append(&mut self.written);
            Ok(())
        }
    }

    fn report_to<W: Write>(mode: Mode, writer: W) -> Report<W> {
        Report::new(Output::new(mode, writer), 0)
    }

    #[test]
    fn report_add_sums_lines_of_each_view() {
        let one = Changed::sample("a").detail.unwrap();
        let mut report = report_to(Mode::Check, Vec::new());
        for outcome in [
            Outcome::Changed(Changed::sample("a")),
            Outcome::Unchanged,
            Outcome::Changed(Changed::sample("b")),
        ] {
            report.add(outcome);
        }
        let counts = report.counts;
        assert_eq!(
            (counts.changed_lines, counts.whitespace_only_lines),
            (2 * one.changed_lines, 2 * one.whitespace_only_lines)
        );
    }

    #[test]
    fn report_add_counts_no_file_for_problem_of_no_file() {
        let mut report = report_to(Mode::Check, Vec::new());
        report.add(Outcome::Unattached(vec![Problem::new("e")]));
        assert_eq!(report.counts, Counts::default());
        assert_eq!(report.problems, [Problem::new("e")]);
    }

    #[test]
    fn report_add_flushes_each_change_before_the_end() {
        let mut report = report_to(Mode::Report(Reporter::Gitlab), Flushed::default());
        report.add(Outcome::Changed(Changed::sample("a")));
        assert_eq!(report.output.writer().flushed, b"[a\n");
        report.add(Outcome::Changed(Changed::sample("b")));
        assert_eq!(report.output.writer().flushed, b"[a\n,b\n");
    }
}
