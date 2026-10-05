use crate::{Mode, style::Style};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct Counts {
    pub(crate) changed: usize,
    pub(crate) unchanged: usize,
    pub(crate) failed: usize,
    pub(crate) skipped: usize,
    pub(crate) excluded: usize,
    pub(crate) changed_lines: usize,
    pub(crate) whitespace_only_lines: usize,
}

impl Counts {
    fn total(&self) -> usize {
        self.changed + self.unchanged + self.failed + self.skipped + self.excluded
    }
}

pub(crate) fn summary(counts: &Counts, mode: Mode, style: Style) -> Option<String> {
    let changed = match (mode, counts.changed) {
        (Mode::Write, _) => "formatted",
        (Mode::Check | Mode::Report(_), 1) => "needs formatting",
        (Mode::Check | Mode::Report(_), _) => "need formatting",
        // Not after a list, which a script reads, as with other formatters.
        (Mode::List, _) => return None,
    };
    let whitespace = match counts.whitespace_only_lines {
        0 => String::new(),
        count => format!(", {count} whitespace only"),
    };
    let lines = match counts.changed_lines {
        0 => String::new(),
        1 => format!(" (1 line{whitespace})"),
        count => format!(" ({count} lines{whitespace})"),
    };
    let changed_part = format!("{changed}{lines}");
    let parts: Vec<_> = [
        (counts.changed, changed_part.as_str()),
        (counts.unchanged, "unchanged"),
        (counts.failed, "failed"),
        (counts.skipped, "skipped"),
        (counts.excluded, "excluded"),
    ]
    .into_iter()
    .filter(|(count, _)| *count > 0)
    .map(|(count, what)| format!("{} {what}", style.bold(&count.to_string())))
    .collect();
    let total = counts.total();
    let files = if total == 1 { "file" } else { "files" };
    let head = format!("{} {files}", style.bold(&total.to_string()));
    if parts.is_empty() {
        Some(head)
    } else {
        Some(format!("{head}: {}", parts.join(", ")))
    }
}

#[cfg(test)]
mod tests {
    use super::{Counts, summary};
    use crate::{Mode, Reporter, style::Style};

    fn plain(counts: Counts, mode: Mode) -> Option<String> {
        summary(&counts, mode, Style::PLAIN)
    }

    #[test]
    fn summary_counts_each_kind_of_file_and_leaves_out_kinds_without_files() {
        let counts = Counts {
            changed: 2,
            unchanged: 0,
            failed: 1,
            skipped: 3,
            excluded: 4,
            changed_lines: 0,
            whitespace_only_lines: 0,
        };
        assert_eq!(
            plain(counts, Mode::Write),
            Some("10 files: 2 formatted, 1 failed, 3 skipped, 4 excluded".to_string())
        );
    }

    #[test]
    fn summary_of_check_says_what_needs_formatting() {
        let one = Counts {
            changed: 1,
            ..Counts::default()
        };
        let two = Counts {
            changed: 2,
            unchanged: 1,
            ..Counts::default()
        };
        assert_eq!(
            plain(one, Mode::Check),
            Some("1 file: 1 needs formatting".to_string())
        );
        assert_eq!(
            plain(one, Mode::Report(Reporter::Github)),
            Some("1 file: 1 needs formatting".to_string())
        );
        assert_eq!(
            plain(two, Mode::Check),
            Some("3 files: 2 need formatting, 1 unchanged".to_string())
        );
    }

    #[test]
    fn summary_of_check_counts_lines_that_change() {
        let one = Counts {
            changed: 1,
            changed_lines: 1,
            ..Counts::default()
        };
        let many = Counts {
            changed: 2,
            changed_lines: 5,
            ..Counts::default()
        };
        assert_eq!(
            plain(one, Mode::Check),
            Some("1 file: 1 needs formatting (1 line)".to_string())
        );
        assert_eq!(
            plain(many, Mode::Check),
            Some("2 files: 2 need formatting (5 lines)".to_string())
        );
    }

    #[test]
    fn summary_of_check_counts_whitespace_only_lines_apart() {
        let one = Counts {
            changed: 1,
            changed_lines: 1,
            whitespace_only_lines: 1,
            ..Counts::default()
        };
        let many = Counts {
            changed: 2,
            changed_lines: 5,
            whitespace_only_lines: 3,
            ..Counts::default()
        };
        assert_eq!(
            plain(one, Mode::Check),
            Some("1 file: 1 needs formatting (1 line, 1 whitespace only)".to_string())
        );
        assert_eq!(
            plain(many, Mode::Check),
            Some("2 files: 2 need formatting (5 lines, 3 whitespace only)".to_string())
        );
    }

    #[test]
    fn summary_is_left_out_for_list() {
        assert_eq!(plain(Counts::default(), Mode::List), None);
    }

    #[test]
    fn summary_of_no_files_is_count_alone() {
        assert_eq!(
            plain(Counts::default(), Mode::Write),
            Some("0 files".to_string())
        );
    }

    #[test]
    fn summary_puts_counts_in_bold_when_colored() {
        let counts = Counts {
            changed: 1,
            ..Counts::default()
        };
        assert_eq!(
            summary(&counts, Mode::Write, Style::COLORED),
            Some("\u{1b}[1m1\u{1b}[0m file: \u{1b}[1m1\u{1b}[0m formatted".to_string())
        );
        assert_eq!(
            summary(&Counts::default(), Mode::Write, Style::COLORED),
            Some("\u{1b}[1m0\u{1b}[0m files".to_string())
        );
    }
}
