use std::ops::RangeInclusive;

use super::{Change, Hunk, hunks};

/// How a file changes by line, which the view of `--check`, the reports, and the summary read.
#[derive(Debug)]
pub(crate) struct Difference<'a> {
    pub(crate) old: &'a str,
    pub(crate) new: &'a str,
    pub(crate) old_lines: Vec<&'a str>,
    pub(crate) new_lines: Vec<&'a str>,
    /// None when only line endings change, since every line would then be a change.
    pub(crate) hunks: Option<Vec<Hunk>>,
}

impl<'a> Difference<'a> {
    pub(crate) fn new(old: &'a str, new: &'a str) -> Difference<'a> {
        let old_lines: Vec<_> = old.split_inclusive('\n').collect();
        let new_lines: Vec<_> = new.split_inclusive('\n').collect();
        let hunks = (old.replace("\r\n", "\n") != new.replace("\r\n", "\n"))
            .then(|| hunks(&old_lines, &new_lines));
        Difference {
            old,
            new,
            old_lines,
            new_lines,
            hunks,
        }
    }

    pub(crate) fn changed_lines(&self) -> usize {
        match &self.hunks {
            Some(_) => self.changes().map(span).sum(),
            None => self.old_lines.len(),
        }
    }

    pub(crate) fn whitespace_only_lines(&self) -> usize {
        match &self.hunks {
            Some(_) => self
                .changes()
                .filter(|change| self.differs_in_whitespace_alone(change))
                .map(span)
                .sum(),
            None => self.old_lines.len(),
        }
    }

    pub(crate) fn changed_span(&self) -> RangeInclusive<usize> {
        let last_line = self.old_lines.len().max(1);
        let line_of = |index: usize| (index + 1).min(last_line);
        match (self.changes().next(), self.changes().last()) {
            (Some(first), Some(last)) => {
                let start = line_of(first.old.start);
                let end = last.old.end.max(line_of(last.old.start));
                start..=end
            },
            _ => 1..=last_line,
        }
    }

    fn changes(&self) -> impl Iterator<Item = &Change> {
        self.hunks.iter().flatten().flat_map(|hunk| &hunk.changes)
    }

    /// Line breaks count as whitespace, since formatting joins and splits lines.
    fn differs_in_whitespace_alone(&self, change: &Change) -> bool {
        let visible = |lines: &[&str]| {
            lines
                .iter()
                .flat_map(|line| line.chars())
                .filter(|ch| !ch.is_whitespace())
                .collect::<String>()
        };
        visible(&self.old_lines[change.old.clone()]) == visible(&self.new_lines[change.new.clone()])
    }
}

fn span(change: &Change) -> usize {
    change.old.len().max(change.new.len())
}

#[cfg(test)]
mod tests {
    use super::Difference;

    fn line_counts(old: &str, new: &str) -> (usize, usize) {
        let difference = Difference::new(old, new);
        (
            difference.changed_lines(),
            difference.whitespace_only_lines(),
        )
    }

    #[test]
    fn difference_counts_lines_of_longer_side_of_each_change() {
        assert_eq!(line_counts("a\nb\nc\n", "x\nc\n").0, 2);
        assert_eq!(line_counts("ls | x\n", "ls\n| x\n").0, 2);
    }

    #[test]
    fn difference_counts_lines_of_changes_in_whitespace_alone() {
        assert_eq!(line_counts("ls|x\n\n\nls\n", "ls | x\n\n\nls -a\n"), (2, 1));
        assert_eq!(line_counts("ls\n| x\n", "ls | x\n"), (2, 2));
    }

    #[test]
    fn difference_counts_every_line_of_change_of_line_endings_alone_as_whitespace() {
        assert_eq!(line_counts("a\r\nb\r\n", "a\nb\n"), (2, 2));
    }

    #[test]
    fn difference_changed_span_covers_old_lines_from_first_change_to_last() {
        let old = "a\nb\nc\nd\ne\nf\ng\nh\ni\nj\n";
        let new = "a\nB\nc\nd\ne\nf\ng\nH\ni\nj\n";
        assert_eq!(Difference::new(old, new).changed_span(), 2..=8);
    }

    #[test]
    fn difference_changed_span_puts_insertion_on_line_it_comes_before() {
        assert_eq!(Difference::new("a\nb\n", "a\nx\nb\n").changed_span(), 2..=2);
    }

    #[test]
    fn difference_changed_span_puts_last_insertion_after_earlier_change_on_line_it_comes_before() {
        assert_eq!(
            Difference::new("a\nb\nc\n", "A\nb\nx\nc\n").changed_span(),
            1..=3
        );
    }

    #[test]
    fn difference_changed_span_puts_insertion_at_end_on_last_line() {
        assert_eq!(Difference::new("a\nb", "a\nb\n").changed_span(), 2..=2);
        assert_eq!(Difference::new("a\n", "a\nb\n").changed_span(), 1..=1);
        assert_eq!(Difference::new("", "a\n").changed_span(), 1..=1);
    }

    #[test]
    fn difference_changed_span_covers_every_line_when_line_endings_alone_change() {
        assert_eq!(
            Difference::new("a\r\nb\r\n", "a\nb\n").changed_span(),
            1..=2
        );
    }
}
