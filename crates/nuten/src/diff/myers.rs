//! Not the plain Myers search, which keeps every round and takes memory quadratic in the edits.

use std::{
    collections::HashMap,
    hash::Hash,
    ops::{Index, IndexMut, Range},
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Change {
    pub(crate) old: Range<usize>,
    pub(crate) new: Range<usize>,
}

/// Not unbounded, since a file changed on every line would cost seconds.
const COST_LIMIT: usize = 256;

pub(crate) fn changes<T: Eq + Hash>(old: &[T], new: &[T]) -> Vec<Change> {
    changes_up_to(old, new, COST_LIMIT)
}

fn changes_up_to<T: Eq + Hash>(old: &[T], new: &[T], cost_limit: usize) -> Vec<Change> {
    let cost_limit = cost_limit.min(old.len() + new.len());
    if old.len() + new.len() <= 2 * cost_limit {
        return searched(old, new, cost_limit, None);
    }
    let mut ids = HashMap::new();
    let old = numbered(old, &mut ids);
    let new = numbered(new, &mut ids);
    let unshared = Unshared::new(&old, &new, ids.len());
    searched(&old, &new, cost_limit, Some(unshared))
}

fn numbered<'a, T: Eq + Hash>(items: &'a [T], ids: &mut HashMap<&'a T, usize>) -> Vec<usize> {
    items
        .iter()
        .map(|item| {
            let next = ids.len();
            *ids.entry(item).or_insert(next)
        })
        .collect()
}

fn searched<T: Eq>(
    old: &[T],
    new: &[T],
    cost_limit: usize,
    unshared: Option<Unshared>,
) -> Vec<Change> {
    let mut search = Search {
        old,
        new,
        forward: Reach::new(cost_limit + 1),
        backward: Reach::new(cost_limit + 1),
        cost_limit,
        unshared,
        changes: Vec::new(),
    };
    search.split(0..old.len(), 0..new.len());
    search.changes
}

struct Search<'a, T> {
    old: &'a [T],
    new: &'a [T],
    forward: Reach,
    backward: Reach,
    cost_limit: usize,
    unshared: Option<Unshared>,
    changes: Vec<Change>,
}

impl<T: Eq> Search<'_, T> {
    /// Not recursion into what follows a split, which can nest deep enough to overflow the stack.
    fn split(&mut self, old: Range<usize>, new: Range<usize>) {
        let mut rest = Some((old, new));
        while let Some((old, new)) = rest {
            rest = self.split_off_head(old, new);
        }
    }

    fn split_off_head(
        &mut self,
        old: Range<usize>,
        new: Range<usize>,
    ) -> Option<(Range<usize>, Range<usize>)> {
        let (old, new) = self.trimmed(old, new);
        let point = (!old.is_empty() && !new.is_empty())
            .then(|| self.split_point(old.clone(), new.clone()))
            .flatten();
        let Some((x, y)) = point else {
            self.push(old, new);
            return None;
        };
        self.split(old.start..x, new.start..y);
        Some((x..old.end, y..new.end))
    }

    fn trimmed(&self, old: Range<usize>, new: Range<usize>) -> (Range<usize>, Range<usize>) {
        let prefix = shared_count(self.old[old.clone()].iter(), self.new[new.clone()].iter());
        let (old, new) = (old.start + prefix..old.end, new.start + prefix..new.end);
        let suffix = shared_count(
            self.old[old.clone()].iter().rev(),
            self.new[new.clone()].iter().rev(),
        );
        (old.start..old.end - suffix, new.start..new.end - suffix)
    }

    fn push(&mut self, old: Range<usize>, new: Range<usize>) {
        if old.is_empty() && new.is_empty() {
            return;
        }
        match self.changes.last_mut() {
            Some(last) if last.old.end == old.start && last.new.end == new.start => {
                last.old.end = old.end;
                last.new.end = new.end;
            },
            _ => self.changes.push(Change {
                old,
                new,
            }),
        }
    }

    fn split_point(&mut self, old: Range<usize>, new: Range<usize>) -> Option<(usize, usize)> {
        let (n, m) = (signed(old.len()), signed(new.len()));
        let delta = n - m;
        let odd = delta % 2 != 0;
        let at = |x: isize, y: isize| (old.start + unsigned(x), new.start + unsigned(y));
        // Not searching back when unshared items cost more than the searches can meet within.
        let can_meet = self
            .unshared
            .as_ref()
            .is_none_or(|unshared| unshared.within(&old, &new) <= 2 * self.cost_limit);
        self.forward[1] = 0;
        self.backward[1] = 0;
        let mut furthest: Option<((isize, isize), _)> = None;
        for d in 0..=signed(self.cost_limit) {
            for k in (-d..=d).step_by(2) {
                let x = self.forward.step(d, k);
                let x = x + self.ahead(&old, &new, x, x - k);
                self.forward[k] = x;
                if can_meet && odd && (delta - k).abs() < d && x + self.backward[delta - k] >= n {
                    return Some(at(x, x - k));
                }
                // Not the lowest diagonal on a tie, which splits past lines both texts keep.
                let rank = (x + (x - k), -(delta - k).abs());
                let inside = x <= n && (0..=m).contains(&(x - k));
                if d > 0 && inside && furthest.is_none_or(|(_, best)| rank > best) {
                    furthest = Some(((x, x - k), rank));
                }
            }
            if !can_meet {
                continue;
            }
            for k in (-d..=d).step_by(2) {
                let x = self.backward.step(d, k);
                let x = x + self.behind(&old, &new, x, x - k);
                self.backward[k] = x;
                if !odd && (delta - k).abs() <= d && x + self.forward[delta - k] >= n {
                    return Some(at(n - x, m - (x - k)));
                }
            }
        }
        furthest.map(|((x, y), _)| at(x, y))
    }

    fn ahead(&self, old: &Range<usize>, new: &Range<usize>, x: isize, y: isize) -> isize {
        let old_rest = self.old[old.clone()].get(unsigned(x)..);
        let new_rest = self.new[new.clone()].get(unsigned(y)..);
        old_rest.zip(new_rest).map_or(0, |(old_rest, new_rest)| {
            signed(shared_count(old_rest.iter(), new_rest.iter()))
        })
    }

    fn behind(&self, old: &Range<usize>, new: &Range<usize>, x: isize, y: isize) -> isize {
        let old_rest = old
            .len()
            .checked_sub(unsigned(x))
            .map(|end| &self.old[old.clone()][..end]);
        let new_rest = new
            .len()
            .checked_sub(unsigned(y))
            .map(|end| &self.new[new.clone()][..end]);
        old_rest.zip(new_rest).map_or(0, |(old_rest, new_rest)| {
            signed(shared_count(old_rest.iter().rev(), new_rest.iter().rev()))
        })
    }
}

/// How many items before each position the other text lacks.
struct Unshared {
    old: Vec<usize>,
    new: Vec<usize>,
}

impl Unshared {
    fn new(old: &[usize], new: &[usize], kinds: usize) -> Unshared {
        let counts = |text: &[usize], other: &[usize]| {
            let mut in_other = vec![false; kinds];
            for &id in other {
                in_other[id] = true;
            }
            let counts = text.iter().scan(0, |count, &id| {
                *count += usize::from(!in_other[id]);
                Some(*count)
            });
            [0].into_iter().chain(counts).collect()
        };
        Unshared {
            old: counts(old, new),
            new: counts(new, old),
        }
    }

    fn within(&self, old: &Range<usize>, new: &Range<usize>) -> usize {
        self.old[old.end] - self.old[old.start] + self.new[new.end] - self.new[new.start]
    }
}

struct Reach {
    x: Vec<isize>,
    offset: isize,
}

impl Reach {
    fn new(bound: usize) -> Reach {
        Reach {
            x: vec![0; 2 * bound + 1],
            offset: signed(bound),
        }
    }

    fn step(&self, d: isize, k: isize) -> isize {
        if k == -d || (k != d && self[k - 1] < self[k + 1]) {
            self[k + 1]
        } else {
            self[k - 1] + 1
        }
    }
}

impl Index<isize> for Reach {
    type Output = isize;

    fn index(&self, k: isize) -> &isize {
        &self.x[unsigned(k + self.offset)]
    }
}

impl IndexMut<isize> for Reach {
    fn index_mut(&mut self, k: isize) -> &mut isize {
        &mut self.x[unsigned(k + self.offset)]
    }
}

fn shared_count<'a, T: Eq + 'a>(
    old: impl Iterator<Item = &'a T>,
    new: impl Iterator<Item = &'a T>,
) -> usize {
    old.zip(new).take_while(|(old, new)| old == new).count()
}

fn signed(value: usize) -> isize {
    isize::try_from(value).expect("a range fits in isize, as a slice does")
}

fn unsigned(value: isize) -> usize {
    usize::try_from(value).expect("a point lies within its ranges")
}

#[cfg(test)]
mod tests {
    use super::{Change, Unshared, changes, changes_up_to, searched};

    fn all_sequences(kinds: u8, max_len: usize) -> Vec<Vec<u8>> {
        (0..max_len).fold(vec![vec![]], |all, _| {
            let longest: Vec<Vec<u8>> = all
                .iter()
                .filter(|sequence| sequence.len() == all.last().map_or(0, Vec::len))
                .flat_map(|sequence| (0..kinds).map(|item| [sequence.as_slice(), &[item]].concat()))
                .collect();
            [all, longest].concat()
        })
    }

    fn all_pairs(kinds: u8, max_len: usize) -> Vec<(Vec<u8>, Vec<u8>)> {
        let all = all_sequences(kinds, max_len);
        all.iter()
            .flat_map(|old| all.iter().map(|new| (old.clone(), new.clone())))
            .collect()
    }

    fn applied<T: Clone>(old: &[T], new: &[T], changes: &[Change]) -> Vec<T> {
        let ends = changes
            .iter()
            .map(|change| change.old.start)
            .chain([old.len()]);
        let starts = [0]
            .into_iter()
            .chain(changes.iter().map(|change| change.old.end));
        let kept = starts.zip(ends).map(|(start, end)| &old[start..end]);
        let replaced = changes
            .iter()
            .map(|change| &new[change.new.clone()])
            .chain([&[][..]]);
        kept.zip(replaced)
            .flat_map(|(kept, replaced)| [kept, replaced])
            .flatten()
            .cloned()
            .collect()
    }

    fn longest_common_len<T: Eq>(old: &[T], new: &[T]) -> usize {
        old.iter()
            .fold(vec![0; new.len() + 1], |above, item| {
                new.iter()
                    .zip(above.windows(2))
                    .fold(vec![0], |row, (other, pair)| {
                        let here = if item == other {
                            pair[0] + 1
                        } else {
                            pair[1].max(*row.last().unwrap())
                        };
                        [row, vec![here]].concat()
                    })
            })
            .last()
            .copied()
            .unwrap()
    }

    fn kept_len(old_len: usize, changes: &[Change]) -> usize {
        old_len - changes.iter().map(|change| change.old.len()).sum::<usize>()
    }

    fn lines(len: usize, changed: impl Fn(usize) -> bool) -> Vec<String> {
        (0..len)
            .map(|line| {
                if changed(line) {
                    format!("{line} changed")
                } else {
                    format!("{line}")
                }
            })
            .collect()
    }

    #[test]
    fn changes_turn_old_into_new() {
        for (old, new) in all_pairs(3, 5) {
            for cost_limit in [0, 1, 2, usize::MAX] {
                let changes = changes_up_to(&old, &new, cost_limit);
                assert_eq!(applied(&old, &new, &changes), new, "{old:?} {new:?}");
            }
        }
    }

    #[test]
    fn unshared_counts_items_other_text_lacks_within_ranges() {
        let unshared = Unshared::new(&[0, 1, 1], &[1, 2], 3);
        assert_eq!(unshared.within(&(0..3), &(0..2)), 2);
        assert_eq!(unshared.within(&(1..3), &(0..1)), 0);
    }

    #[test]
    fn changes_match_search_without_numbering_or_skipping() {
        let longer = [
            (
                vec![1, 0, 0, 0, 0, 0, 0, 1, 1, 1, 0, 0],
                vec![0, 0, 0, 0, 2, 2, 2, 2, 0, 2],
            ),
            (vec![0, 0, 0, 0, 1], vec![1, 2, 2, 1, 2, 2]),
        ];
        for (old, new) in all_pairs(3, 5).into_iter().chain(longer) {
            for cost_limit in [0, 1, 2, 3] {
                assert_eq!(
                    changes_up_to(&old, &new, cost_limit),
                    searched(&old, &new, cost_limit, None),
                    "{old:?} {new:?} {cost_limit}"
                );
            }
        }
    }

    #[test]
    fn changes_keep_longest_common_subsequence() {
        for (old, new) in all_pairs(3, 5) {
            let changes = changes(&old, &new);
            assert_eq!(
                kept_len(old.len(), &changes),
                longest_common_len(&old, &new),
                "{old:?} {new:?} {changes:?}"
            );
        }
    }

    #[test]
    fn changes_stand_apart_in_order_and_change_something() {
        for (old, new) in all_pairs(3, 5) {
            for cost_limit in [0, 1, 2, usize::MAX] {
                let changes = changes_up_to(&old, &new, cost_limit);
                let apart = changes.windows(2).all(|pair| {
                    let old_gap = pair[1].old.start.checked_sub(pair[0].old.end);
                    let new_gap = pair[1].new.start.checked_sub(pair[0].new.end);
                    matches!((old_gap, new_gap), (Some(old), Some(new)) if old > 0 && old == new)
                });
                let nonempty = changes
                    .iter()
                    .all(|change| !change.old.is_empty() || !change.new.is_empty());
                assert!(apart && nonempty, "{old:?} {new:?} {changes:?}");
            }
        }
    }

    #[test]
    fn changes_keep_longest_common_subsequence_within_cost_limit() {
        let old: Vec<usize> = (0..240).collect();
        let new: Vec<usize> = (120..240).chain(0..120).collect();
        assert_eq!(kept_len(old.len(), &changes(&old, &new)), 120);
    }

    #[test]
    fn changes_past_cost_limit_keep_all_of_shorter_text() {
        let short = [0, 1];
        let long = [1, 1, 1, 0, 0, 0, 0, 0, 0, 1, 1];
        assert_eq!(
            kept_len(short.len(), &changes_up_to(&short, &long, 1)),
            short.len()
        );
        assert_eq!(
            kept_len(long.len(), &changes_up_to(&long, &short, 1)),
            short.len()
        );
    }

    #[test]
    fn changes_past_cost_limit_cover_only_changed_blocks() {
        let old = lines(2000, |_| false);
        let new = lines(2000, |line| {
            (100..300).contains(&line) || (1700..1900).contains(&line)
        });
        assert_eq!(
            changes_up_to(&old, &new, 4),
            [
                Change {
                    old: 100..300,
                    new: 100..300,
                },
                Change {
                    old: 1700..1900,
                    new: 1700..1900,
                },
            ]
        );
    }

    #[test]
    fn changes_past_cost_limit_cover_only_scattered_changed_lines() {
        let old = lines(2000, |_| false);
        let new = lines(2000, |line| line % 10 == 0);
        let changes = changes_up_to(&old, &new, 4);
        assert_eq!(changes.len(), 200);
        assert!(
            changes
                .iter()
                .all(|change| change.old.len() == 1 && change.new.len() == 1)
        );
    }
}
