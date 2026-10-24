mod difference;
mod myers;

use std::{hash::Hash, ops::Range};

pub(crate) use self::{
    difference::Difference,
    myers::{Change, changes},
};

const CONTEXT: usize = 3;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Hunk {
    pub(crate) old: Range<usize>,
    pub(crate) changes: Vec<Change>,
}

pub(crate) fn hunks<T: Eq + Hash>(old: &[T], new: &[T]) -> Vec<Hunk> {
    let changes = changes(old, new);
    let breaks = changes
        .windows(2)
        .enumerate()
        .filter(|(_, pair)| pair[1].old.start - pair[0].old.end > 2 * CONTEXT)
        .map(|(index, _)| index + 1);
    let starts = [0].into_iter().chain(breaks.clone());
    let ends = breaks.chain([changes.len()]);
    starts
        .zip(ends)
        .filter(|(start, end)| start < end)
        .map(|(start, end)| {
            let group = &changes[start..end];
            let (first, last) = (&group[0], &group[group.len() - 1]);
            let before = first.old.start.min(CONTEXT);
            let after = (old.len() - last.old.end).min(CONTEXT);
            Hunk {
                old: first.old.start - before..last.old.end + after,
                changes: group.to_vec(),
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use std::ops::Range;

    use super::hunks;

    fn lines_changed_at(len: usize, changed: &[usize]) -> Vec<String> {
        (0..len)
            .map(|line| {
                if changed.contains(&line) {
                    format!("{line} changed")
                } else {
                    line.to_string()
                }
            })
            .collect()
    }

    fn hunk_shapes(len: usize, changed: &[usize]) -> Vec<(Range<usize>, usize)> {
        hunks(&lines_changed_at(len, &[]), &lines_changed_at(len, changed))
            .into_iter()
            .map(|hunk| (hunk.old, hunk.changes.len()))
            .collect()
    }

    #[test]
    fn same_lines_give_no_hunk() {
        assert_eq!(hunk_shapes(10, &[]), []);
    }

    #[test]
    fn hunk_holds_three_lines_of_context_each_side() {
        assert_eq!(hunk_shapes(10, &[5]), [(2..9, 1)]);
    }

    #[test]
    fn context_stops_at_first_and_last_line() {
        assert_eq!(hunk_shapes(10, &[1]), [(0..5, 1)]);
        assert_eq!(hunk_shapes(10, &[8]), [(5..10, 1)]);
    }

    #[test]
    fn changes_six_lines_apart_share_hunk() {
        assert_eq!(hunk_shapes(20, &[4, 11]), [(1..15, 2)]);
    }

    #[test]
    fn changes_seven_lines_apart_take_own_hunks() {
        assert_eq!(hunk_shapes(20, &[4, 12]), [(1..8, 1), (9..16, 1)]);
    }

    #[test]
    fn hunk_of_insertion_alone_spans_context_around_it() {
        let old = lines_changed_at(4, &[]);
        let new = [&old[..2], &["new".to_string()], &old[2..]].concat();
        let hunks = hunks(&old, &new);
        assert_eq!(hunks.len(), 1);
        assert_eq!(hunks[0].old, 0..4);
    }
}
