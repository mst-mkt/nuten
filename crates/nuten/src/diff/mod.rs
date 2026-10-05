mod myers;

use std::ops::Range;

use self::myers::{Change, changes};

const CONTEXT: usize = 3;

/// Prefixes `path` with `a/` and `b/`, which `git apply` strips by default.
pub(crate) fn unified(path: &str, old: &str, new: &str) -> String {
    let old_lines: Vec<_> = old.split_inclusive('\n').collect();
    let new_lines: Vec<_> = new.split_inclusive('\n').collect();
    let changes = changes(&old_lines, &new_lines);
    if changes.is_empty() {
        return String::new();
    }
    let header = format!("--- a/{path}\n+++ b/{path}\n");
    let hunks = hunks(&changes, old_lines.len())
        .map(|hunk| hunk_text(&hunk, &old_lines, &new_lines))
        .collect::<String>();
    header + &hunks
}

struct Hunk<'a> {
    old: Range<usize>,
    new: Range<usize>,
    changes: &'a [Change],
}

fn hunks(changes: &[Change], old_len: usize) -> impl Iterator<Item = Hunk<'_>> {
    let breaks = changes
        .windows(2)
        .enumerate()
        .filter(|(_, pair)| pair[1].old.start - pair[0].old.end > 2 * CONTEXT)
        .map(|(index, _)| index + 1);
    let starts = [0].into_iter().chain(breaks.clone());
    let ends = breaks.chain([changes.len()]);
    starts.zip(ends).map(move |(start, end)| {
        let group = &changes[start..end];
        let (first, last) = (&group[0], &group[group.len() - 1]);
        let before = first.old.start.min(CONTEXT);
        let after = (old_len - last.old.end).min(CONTEXT);
        Hunk {
            old: first.old.start - before..last.old.end + after,
            new: first.new.start - before..last.new.end + after,
            changes: group,
        }
    })
}

fn hunk_text(hunk: &Hunk<'_>, old_lines: &[&str], new_lines: &[&str]) -> String {
    let header = format!(
        "@@ -{} +{} @@\n",
        range_text(&hunk.old),
        range_text(&hunk.new)
    );
    header + &body_text(hunk, old_lines, new_lines)
}

fn body_text(hunk: &Hunk<'_>, old_lines: &[&str], new_lines: &[&str]) -> String {
    let kept_starts = [hunk.old.start]
        .into_iter()
        .chain(hunk.changes.iter().map(|change| change.old.end));
    let kept_ends = hunk
        .changes
        .iter()
        .map(|change| change.old.start)
        .chain([hunk.old.end]);
    let kept = kept_starts
        .zip(kept_ends)
        .map(|(start, end)| lines_text(" ", &old_lines[start..end]));
    let replaced = hunk
        .changes
        .iter()
        .map(|change| {
            lines_text("-", &old_lines[change.old.clone()])
                + &lines_text("+", &new_lines[change.new.clone()])
        })
        .chain([String::new()]);
    kept.zip(replaced)
        .flat_map(|(kept, replaced)| [kept, replaced])
        .collect()
}

fn range_text(range: &Range<usize>) -> String {
    match range.len() {
        0 => format!("{},0", range.start),
        1 => format!("{}", range.start + 1),
        len => format!("{},{len}", range.start + 1),
    }
}

fn lines_text(sign: &str, lines: &[&str]) -> String {
    lines
        .iter()
        .flat_map(|line| {
            let no_newline_mark = if line.ends_with('\n') {
                ""
            } else {
                "\n\\ No newline at end of file\n"
            };
            [sign, line, no_newline_mark]
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::unified;

    fn numbered(len: usize, replaced: &[(usize, &str)]) -> String {
        (0..len)
            .map(
                |line| match replaced.iter().find(|(index, _)| *index == line) {
                    Some((_, text)) => format!("{text}\n"),
                    None => format!("{line}\n"),
                },
            )
            .collect()
    }

    fn joined(lines: &[&str]) -> String {
        lines.iter().flat_map(|line| [line, "\n"]).collect()
    }

    #[test]
    fn same_texts_give_no_diff() {
        assert_eq!(unified("a.nu", "ls\n", "ls\n"), "");
    }

    #[test]
    fn change_shows_three_lines_of_context_each_side() {
        let old = numbered(10, &[]);
        let new = numbered(10, &[(5, "five")]);
        assert_eq!(
            unified("a.nu", &old, &new),
            joined(&[
                "--- a/a.nu",
                "+++ b/a.nu",
                "@@ -3,7 +3,7 @@",
                " 2",
                " 3",
                " 4",
                "-5",
                "+five",
                " 6",
                " 7",
                " 8",
            ])
        );
    }

    #[test]
    fn context_stops_at_start_of_file() {
        let old = numbered(10, &[]);
        let new = numbered(10, &[(1, "one")]);
        assert_eq!(
            unified("a.nu", &old, &new),
            joined(&[
                "--- a/a.nu",
                "+++ b/a.nu",
                "@@ -1,5 +1,5 @@",
                " 0",
                "-1",
                "+one",
                " 2",
                " 3",
                " 4",
            ])
        );
    }

    #[test]
    fn context_stops_at_end_of_file() {
        let old = numbered(10, &[]);
        let new = numbered(10, &[(8, "eight")]);
        assert_eq!(
            unified("a.nu", &old, &new),
            joined(&[
                "--- a/a.nu",
                "+++ b/a.nu",
                "@@ -6,5 +6,5 @@",
                " 5",
                " 6",
                " 7",
                "-8",
                "+eight",
                " 9",
            ])
        );
    }

    #[test]
    fn changes_six_lines_apart_share_hunk() {
        let old = numbered(20, &[]);
        let new = numbered(20, &[(4, "four"), (11, "eleven")]);
        assert_eq!(
            unified("a.nu", &old, &new),
            joined(&[
                "--- a/a.nu",
                "+++ b/a.nu",
                "@@ -2,14 +2,14 @@",
                " 1",
                " 2",
                " 3",
                "-4",
                "+four",
                " 5",
                " 6",
                " 7",
                " 8",
                " 9",
                " 10",
                "-11",
                "+eleven",
                " 12",
                " 13",
                " 14",
            ])
        );
    }

    #[test]
    fn changes_seven_lines_apart_take_own_hunks() {
        let old = numbered(20, &[]);
        let new = numbered(20, &[(4, "four"), (12, "twelve")]);
        assert_eq!(
            unified("a.nu", &old, &new),
            joined(&[
                "--- a/a.nu",
                "+++ b/a.nu",
                "@@ -2,7 +2,7 @@",
                " 1",
                " 2",
                " 3",
                "-4",
                "+four",
                " 5",
                " 6",
                " 7",
                "@@ -10,7 +10,7 @@",
                " 9",
                " 10",
                " 11",
                "-12",
                "+twelve",
                " 13",
                " 14",
                " 15",
            ])
        );
    }

    #[test]
    fn deleted_lines_come_before_inserted_lines() {
        assert_eq!(
            unified("a.nu", "a\nb\n", "c\nd\n"),
            joined(&[
                "--- a/a.nu",
                "+++ b/a.nu",
                "@@ -1,2 +1,2 @@",
                "-a",
                "-b",
                "+c",
                "+d",
            ])
        );
    }

    #[test]
    fn empty_range_names_line_before_it() {
        assert_eq!(
            unified("a.nu", "", "ls\n"),
            joined(&["--- a/a.nu", "+++ b/a.nu", "@@ -0,0 +1 @@", "+ls"])
        );
        assert_eq!(
            unified("a.nu", "ls\n", ""),
            joined(&["--- a/a.nu", "+++ b/a.nu", "@@ -1 +0,0 @@", "-ls"])
        );
    }

    #[test]
    fn line_without_break_is_marked() {
        assert_eq!(
            unified("a.nu", "ls|first", "ls | first\n"),
            joined(&[
                "--- a/a.nu",
                "+++ b/a.nu",
                "@@ -1 +1 @@",
                "-ls|first",
                "\\ No newline at end of file",
                "+ls | first",
            ])
        );
        assert_eq!(
            unified("a.nu", "a", "b"),
            joined(&[
                "--- a/a.nu",
                "+++ b/a.nu",
                "@@ -1 +1 @@",
                "-a",
                "\\ No newline at end of file",
                "+b",
                "\\ No newline at end of file",
            ])
        );
        assert_eq!(
            unified("a.nu", "a\nb", "x\nb"),
            joined(&[
                "--- a/a.nu",
                "+++ b/a.nu",
                "@@ -1,2 +1,2 @@",
                "-a",
                "+x",
                " b",
                "\\ No newline at end of file",
            ])
        );
    }

    #[test]
    fn carriage_return_stays_in_its_line() {
        assert_eq!(
            unified("a.nu", "ls\r\n", "ls\n"),
            joined(&["--- a/a.nu", "+++ b/a.nu", "@@ -1 +1 @@", "-ls\r", "+ls"])
        );
    }
}
