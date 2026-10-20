//! Not globset, which has no `{num1..num2}` and reads `{single}` and `[a/b]` unlike editorconfig.

use std::mem;

/// Not backtracking, which takes exponential time on some globs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Glob {
    states: Vec<Vec<(Step, usize)>>,
    accept: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Step {
    Empty,
    Char(char),
    AnyInName,
    Any,
    Class {
        negated: bool,
        ranges: Vec<(char, char)>,
    },
    NumberRange {
        min: i64,
        max: i64,
    },
}

/// What is read as a character, since nothing closes or follows it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Unclosed {
    Class,
    Group,
    Escape,
}

impl Glob {
    pub(crate) fn new(section: &str) -> Glob {
        Glob::read(section).0
    }

    pub(crate) fn closed(section: &str) -> Result<Glob, Unclosed> {
        match Glob::read(section) {
            (glob, None) => Ok(glob),
            (_, Some(unclosed)) => Err(unclosed),
        }
    }

    fn read(section: &str) -> (Glob, Option<Unclosed>) {
        let anchored = section.contains('/');
        let section = section.strip_prefix('/').unwrap_or(section);
        let chars: Vec<char> = section.chars().collect();
        let mut automaton = Automaton {
            states: vec![Vec::new()],
            end: 0,
        };
        if !anchored {
            automaton.any_dirs();
        }
        let (automaton, unclosed) = Reader {
            braces: braces(&chars),
            chars: &chars,
            groups: Vec::new(),
            after_slash: true,
            unclosed: None,
            automaton,
        }
        .read();
        (automaton.finish(), unclosed)
    }

    /// `path` has `/` between names.
    pub(crate) fn matches(&self, path: &str) -> bool {
        let chars: Vec<char> = path.chars().collect();
        // One bit per pair of a place and a state, since a long path makes many places.
        let words = self.states.len().div_ceil(64);
        let slot = |at: usize, state: usize| (at * words + state / 64, 1_u64 << (state % 64));
        let mut reached = vec![0_u64; (chars.len() + 1) * words];
        let mut pending = vec![(0, 0)];
        while let Some((at, state)) = pending.pop() {
            for (step, next) in &self.states[state] {
                for to in ends_after(step, &chars, at) {
                    let (word, bit) = slot(to, *next);
                    if reached[word] & bit == 0 {
                        reached[word] |= bit;
                        pending.push((to, *next));
                    }
                }
            }
        }
        let (word, bit) = slot(chars.len(), self.accept);
        reached[word] & bit != 0
    }
}

fn ends_after<'a>(
    step: &'a Step,
    chars: &'a [char],
    at: usize,
) -> impl Iterator<Item = usize> + 'a {
    let single = |fits: &dyn Fn(char) -> bool| {
        chars
            .get(at)
            .is_some_and(|&char| fits(char))
            .then_some(at + 1)
    };
    let end = match step {
        Step::Empty => Some(at),
        Step::Char(expected) => single(&|char| char == *expected),
        Step::AnyInName => single(&|char| char != '/'),
        Step::Any => single(&|_| true),
        Step::Class {
            negated,
            ranges,
        } => single(&|char| {
            let within = ranges
                .iter()
                .any(|&(low, high)| (low..=high).contains(&char));
            char != '/' && within != *negated
        }),
        Step::NumberRange {
            ..
        } => None,
    };
    let number_ends = match *step {
        Step::NumberRange {
            min,
            max,
        } => Some(
            integer_ends(&chars[at..])
                .filter(move |(number, _)| (min..=max).contains(number))
                .map(move |(_, length)| at + length),
        ),
        _ => None,
    };
    end.into_iter().chain(number_ends.into_iter().flatten())
}

fn integer_ends(chars: &[char]) -> impl Iterator<Item = (i64, usize)> + '_ {
    let negative = chars.first() == Some(&'-');
    let sign = usize::from(matches!(chars.first(), Some('+' | '-')));
    let digits: Vec<i64> = chars[sign..]
        .iter()
        .map_while(|char| char.to_digit(10))
        .map(i64::from)
        .collect();
    // Not a number written with a leading zero, which no editorconfig core matches.
    let longest = if digits.first() == Some(&0) {
        1
    } else {
        digits.len()
    };
    // Negative numbers are summed as negative, since `i64::MIN` has no positive counterpart.
    digits
        .into_iter()
        .take(longest)
        .scan(0_i64, move |value, digit| {
            let shifted = value.checked_mul(10)?;
            *value = if negative {
                shifted.checked_sub(digit)?
            } else {
                shifted.checked_add(digit)?
            };
            Some(*value)
        })
        .zip(sign + 1..)
}

/// Worked out before the glob is read, since a `{` opens a group only when a `}` closes it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Brace {
    Open { close: usize, group: Group },
    Comma { open: usize },
    Close { open: usize },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Group {
    Alternatives,
    Characters,
    NumberRange { min: i64, max: i64 },
}

fn braces(chars: &[char]) -> Vec<Option<Brace>> {
    let mut braces = vec![None; chars.len()];
    let mut opens: Vec<(usize, bool)> = Vec::new();
    let mut at = 0;
    while let Some(&char) = chars.get(at) {
        match char {
            '\\' => at += 1,
            '{' => opens.push((at, false)),
            ',' => {
                if let Some((open, has_comma)) = opens.last_mut() {
                    *has_comma = true;
                    braces[at] = Some(Brace::Comma {
                        open: *open,
                    });
                }
            },
            '}' => {
                if let Some((open, has_comma)) = opens.pop() {
                    let group = match number_range(&chars[open + 1..at]) {
                        Some((min, max)) => Group::NumberRange {
                            min,
                            max,
                        },
                        None if has_comma => Group::Alternatives,
                        None => Group::Characters,
                    };
                    braces[open] = Some(Brace::Open {
                        close: at,
                        group,
                    });
                    braces[at] = Some(Brace::Close {
                        open,
                    });
                }
            },
            _ => {},
        }
        at += 1;
    }
    braces
}

fn number_range(inner: &[char]) -> Option<(i64, i64)> {
    // Checked first, since collecting in every enclosing group would make nesting quadratic.
    if !inner
        .iter()
        .all(|char| char.is_ascii_digit() || matches!(char, '+' | '-' | '.'))
    {
        return None;
    }
    let inner: String = inner.iter().collect();
    let (min, max) = inner.split_once("..")?;
    let integer = |text: &str| {
        let digits = text.strip_prefix(['+', '-']).unwrap_or(text);
        digits
            .bytes()
            .all(|byte| byte.is_ascii_digit())
            .then(|| text.parse().ok())
            .flatten()
    };
    Some((integer(min)?, integer(max)?))
}

struct Automaton {
    states: Vec<Vec<(Step, usize)>>,
    end: usize,
}

impl Automaton {
    fn finish(self) -> Glob {
        Glob {
            states: self.states,
            accept: self.end,
        }
    }

    fn state(&mut self) -> usize {
        self.states.push(Vec::new());
        self.states.len() - 1
    }

    fn edge(&mut self, from: usize, step: Step, to: usize) {
        self.states[from].push((step, to));
    }

    fn push(&mut self, step: Step) {
        let next = self.state();
        self.edge(self.end, step, next);
        self.end = next;
    }

    fn repeat(&mut self, step: Step) {
        let loop_state = self.state();
        self.edge(self.end, Step::Empty, loop_state);
        self.edge(loop_state, step, loop_state);
        self.end = loop_state;
    }

    fn alternative(&mut self, start: usize) {
        let first = self.state();
        self.edge(start, Step::Empty, first);
        self.end = first;
    }

    fn any_dirs(&mut self) {
        let (start, within, next) = (self.end, self.state(), self.state());
        self.edge(start, Step::Empty, next);
        self.edge(start, Step::Any, within);
        self.edge(within, Step::Any, within);
        self.edge(within, Step::Char('/'), next);
        self.end = next;
    }
}

struct Reader<'a> {
    chars: &'a [char],
    braces: Vec<Option<Brace>>,
    groups: Vec<OpenGroup>,
    after_slash: bool,
    unclosed: Option<Unclosed>,
    automaton: Automaton,
}

struct OpenGroup {
    brace: usize,
    alternatives: Option<Fork>,
}

struct Fork {
    start: usize,
    join: usize,
}

impl Reader<'_> {
    fn read(mut self) -> (Automaton, Option<Unclosed>) {
        let mut at = 0;
        while at < self.chars.len() {
            at = self.step(at);
        }
        (self.automaton, self.unclosed)
    }

    fn note_unclosed(&mut self, unclosed: Unclosed) {
        self.unclosed.get_or_insert(unclosed);
    }

    fn step(&mut self, at: usize) -> usize {
        let top = self.groups.last().map(|group| group.brace);
        let after_slash = mem::replace(&mut self.after_slash, false);
        match (self.chars[at], self.braces[at]) {
            ('\\', _) => self.escaped(at),
            ('*', _) => self.stars(at, after_slash),
            ('?', _) => {
                self.automaton.push(Step::AnyInName);
                at + 1
            },
            ('[', _) => self.class_or_bracket(at),
            (
                '{',
                Some(Brace::Open {
                    close,
                    group,
                }),
            ) => self.open(at, close, group),
            (
                ',',
                Some(Brace::Comma {
                    open,
                }),
            ) if top == Some(open) => self.next_alternative(at),
            (
                '}',
                Some(Brace::Close {
                    open,
                }),
            ) if top == Some(open) => self.close(at),
            ('{', None) => {
                self.note_unclosed(Unclosed::Group);
                self.char('{');
                at + 1
            },
            (char, _) => {
                self.char(char);
                at + 1
            },
        }
    }

    fn char(&mut self, char: char) {
        self.automaton.push(Step::Char(char));
        self.after_slash = char == '/';
    }

    fn escaped(&mut self, at: usize) -> usize {
        let escaped = if let Some(&escaped) = self.chars.get(at + 1) {
            escaped
        } else {
            self.note_unclosed(Unclosed::Escape);
            '\\'
        };
        self.char(escaped);
        at + 2
    }

    fn stars(&mut self, at: usize, after_slash: bool) -> usize {
        let stars = self.chars[at..]
            .iter()
            .take_while(|&&char| char == '*')
            .count();
        if stars == 1 {
            self.automaton.repeat(Step::AnyInName);
        } else if after_slash && self.chars.get(at + stars) == Some(&'/') {
            self.automaton.any_dirs();
            return at + stars + 1;
        } else {
            self.automaton.repeat(Step::Any);
        }
        at + stars
    }

    fn class_or_bracket(&mut self, at: usize) -> usize {
        let bound = self.group_end(at);
        let (step, next) = if let Some(class) = class(&self.chars[..bound], at) {
            class
        } else {
            self.note_unclosed(Unclosed::Class);
            (Step::Char('['), at + 1)
        };
        self.automaton.push(step);
        next
    }

    /// A class ends within its group, as each alternative is read alone.
    fn group_end(&self, at: usize) -> usize {
        let Some(top) = self.groups.last().map(|group| group.brace) else {
            return self.chars.len();
        };
        self.braces[at..]
            .iter()
            .position(|brace| match brace {
                Some(
                    Brace::Comma {
                        open,
                    }
                    | Brace::Close {
                        open,
                    },
                ) => *open == top,
                _ => false,
            })
            .map_or(self.chars.len(), |offset| at + offset)
    }

    fn open(&mut self, at: usize, close: usize, group: Group) -> usize {
        let alternatives = match group {
            Group::NumberRange {
                min,
                max,
            } => {
                self.automaton.push(Step::NumberRange {
                    min,
                    max,
                });
                return close + 1;
            },
            Group::Characters => {
                self.automaton.push(Step::Char('{'));
                None
            },
            Group::Alternatives => {
                let fork = Fork {
                    start: self.automaton.end,
                    join: self.automaton.state(),
                };
                self.automaton.alternative(fork.start);
                Some(fork)
            },
        };
        self.groups.push(OpenGroup {
            brace: at,
            alternatives,
        });
        at + 1
    }

    fn next_alternative(&mut self, at: usize) -> usize {
        let Fork {
            start,
            join,
        } = *self
            .groups
            .last()
            .and_then(|group| group.alternatives.as_ref())
            .expect("a comma of a group is read only in its alternatives");
        self.automaton.edge(self.automaton.end, Step::Empty, join);
        self.automaton.alternative(start);
        at + 1
    }

    fn close(&mut self, at: usize) -> usize {
        let group = self.groups.pop().expect("a group is open");
        match group.alternatives {
            Some(Fork {
                join, ..
            }) => {
                self.automaton.edge(self.automaton.end, Step::Empty, join);
                self.automaton.end = join;
            },
            None => self.automaton.push(Step::Char('}')),
        }
        at + 1
    }
}

/// Not across a `/`, since a class matches within a name.
fn class(chars: &[char], open: usize) -> Option<(Step, usize)> {
    let negated = chars.get(open + 1) == Some(&'!');
    let start = open + 1 + usize::from(negated);
    let mut members = Vec::new();
    let mut at = start;
    let close = loop {
        match *chars.get(at)? {
            ']' if at > start => break at,
            '/' => return None,
            '\\' => {
                members.push((*chars.get(at + 1)?, true));
                at += 2;
            },
            char => {
                members.push((char, false));
                at += 1;
            },
        }
    };
    Some((
        Step::Class {
            negated,
            ranges: ranges(&members),
        },
        close + 1,
    ))
}

fn ranges(members: &[(char, bool)]) -> Vec<(char, char)> {
    let mut ranges = Vec::new();
    let mut at = 0;
    while let Some(&(first, _)) = members.get(at) {
        let (last, length) = match members.get(at + 1..at + 3) {
            Some(&[('-', false), (last, _)]) => (last, 3),
            _ => (first, 1),
        };
        ranges.push((first, last));
        at += length;
    }
    ranges
}

#[cfg(test)]
mod tests {
    use std::iter;

    use super::Glob;

    fn matching<'a>(section: &str, paths: &[&'a str]) -> Vec<&'a str> {
        let glob = Glob::new(section);
        paths
            .iter()
            .copied()
            .filter(|path| glob.matches(path))
            .collect()
    }

    #[test]
    fn section_without_slash_matches_name_at_any_depth() {
        assert_eq!(
            matching("*.nu", &["a.nu", "d/a.nu", "d/e/a.nu", "a.nu/b", "a.nuon"]),
            ["a.nu", "d/a.nu", "d/e/a.nu"]
        );
    }

    #[test]
    fn section_with_slash_matches_path_from_directory_of_file() {
        let paths = ["d/a.nu", "e/d/a.nu", "d/e/a.nu"];
        assert_eq!(matching("d/a.nu", &paths), ["d/a.nu"]);
        assert_eq!(matching("/d/a.nu", &paths), ["d/a.nu"]);
        assert_eq!(matching("d/*.nu", &paths), ["d/a.nu"]);
        assert_eq!(matching("/a.nu", &["a.nu", "d/a.nu"]), ["a.nu"]);
    }

    #[test]
    fn double_star_matches_across_directories() {
        let paths = ["a.nu", "d/a.nu", "d/e/a.nu"];
        assert_eq!(matching("d/**", &paths), ["d/a.nu", "d/e/a.nu"]);
        assert_eq!(matching("**.nu", &paths), paths);
        assert_eq!(matching("/**.nu", &paths), paths);
    }

    #[test]
    fn double_star_between_slashes_matches_no_directory_too() {
        let paths = ["d/a.nu", "d/e/a.nu", "d/e/f/a.nu", "da.nu"];
        assert_eq!(
            matching("d/**/a.nu", &paths),
            ["d/a.nu", "d/e/a.nu", "d/e/f/a.nu"]
        );
        assert_eq!(matching("**/a.nu", &["a.nu", "d/a.nu"]), ["a.nu", "d/a.nu"]);
        assert_eq!(
            matching("/**/a.nu", &["a.nu", "d/a.nu"]),
            ["a.nu", "d/a.nu"]
        );
    }

    #[test]
    fn escaped_slash_before_double_star_matches_no_directory_too() {
        assert_eq!(
            matching(r"a\/**/b", &["a/b", "a/x/b", "ab"]),
            ["a/b", "a/x/b"]
        );
    }

    #[test]
    fn double_star_after_name_still_needs_following_slash() {
        assert_eq!(
            matching("a**/b", &["ab", "a/b", "ax/b", "a/x/b"]),
            ["a/b", "ax/b", "a/x/b"]
        );
    }

    #[test]
    fn double_star_after_wildcard_still_needs_following_slash() {
        assert_eq!(matching("a/?**/b", &["a/xb", "a/x/b"]), ["a/x/b"]);
    }

    #[test]
    fn question_mark_matches_one_character_but_slash() {
        assert_eq!(
            matching("a?.nu", &["ab.nu", "a.nu", "abc.nu", "a日.nu"]),
            ["ab.nu", "a日.nu"]
        );
        assert_eq!(matching("d?a.nu", &["d/a.nu"]), Vec::<&str>::new());
    }

    #[test]
    fn class_matches_one_listed_character() {
        let paths = ["a.nu", "b.nu", "c.nu", "-.nu"];
        assert_eq!(matching("[ab].nu", &paths), ["a.nu", "b.nu"]);
        assert_eq!(matching("[a-b].nu", &paths), ["a.nu", "b.nu"]);
        assert_eq!(matching("[!ab].nu", &paths), ["c.nu", "-.nu"]);
        assert_eq!(matching("[a-].nu", &paths), ["a.nu", "-.nu"]);
        assert_eq!(matching("[]].nu", &["].nu", "a.nu"]), ["].nu"]);
    }

    #[test]
    fn negated_class_matches_no_slash() {
        assert_eq!(matching("a[!b]c", &["a/c", "axc"]), ["axc"]);
    }

    #[test]
    fn escaped_dash_in_class_is_member() {
        assert_eq!(matching(r"[a\-c]", &["a", "b", "c", "-"]), ["a", "c", "-"]);
    }

    #[test]
    fn bracket_with_slash_inside_is_character() {
        assert_eq!(matching("[a/b]", &["[a/b]", "a", "b"]), ["[a/b]"]);
    }

    #[test]
    fn unclosed_bracket_is_character() {
        assert_eq!(matching("[a.nu", &["[a.nu", "a.nu"]), ["[a.nu"]);
    }

    #[test]
    fn class_ends_within_its_alternative() {
        assert_eq!(
            matching("{[a,b]}", &["[a", "b]", "a", "b", ","]),
            ["[a", "b]"]
        );
    }

    #[test]
    fn class_spans_nested_braces_within_its_alternative() {
        assert_eq!(matching("{x,[a{b,c}]}", &["b", "[ab]", "x"]), ["b", "x"]);
    }

    #[test]
    fn class_ends_within_braces_without_comma() {
        assert_eq!(matching("{[a}]", &["{[a}]", "{a"]), ["{[a}]"]);
    }

    #[test]
    fn braces_match_any_alternative() {
        let paths = ["a.nu", "a.nuon", "a.", "a.sh"];
        assert_eq!(matching("a.{nu,nuon}", &paths), ["a.nu", "a.nuon"]);
        assert_eq!(matching("a.{nu,}", &paths), ["a.nu", "a."]);
        assert_eq!(
            matching("a.{n{u,uon},sh}", &paths),
            ["a.nu", "a.nuon", "a.sh"]
        );
        assert_eq!(
            matching("{*.nu,d/*.sh}", &["a.nu", "d/a.sh", "e/a.nu"]),
            ["a.nu", "d/a.sh"]
        );
    }

    #[test]
    fn braces_without_comma_are_characters() {
        assert_eq!(matching("{nu}.nu", &["{nu}.nu", "nu.nu"]), ["{nu}.nu"]);
        assert_eq!(matching("{}.nu", &["{}.nu", ".nu"]), ["{}.nu"]);
        assert_eq!(matching("{*}.nu", &["{a}.nu", "a.nu"]), ["{a}.nu"]);
    }

    #[test]
    fn unclosed_brace_is_character() {
        assert_eq!(matching("{a,b.nu", &["{a,b.nu", "a.nu"]), ["{a,b.nu"]);
    }

    #[test]
    fn escaped_brace_inside_braces_is_character() {
        assert_eq!(
            matching(r"{a\},b}.nu", &["a}.nu", "b.nu", "a.nu"]),
            ["a}.nu", "b.nu"]
        );
    }

    #[test]
    fn number_range_matches_integer_within_it() {
        let paths = ["a0", "a1", "a3", "a4", "a10", "a-1", "a+2", "a01", "a"];
        assert_eq!(matching("a{1..3}", &paths), ["a1", "a3", "a+2"]);
        assert_eq!(matching("a{-1..0}", &paths), ["a0", "a-1"]);
        assert_eq!(matching("a{1..3}0", &paths), ["a10"]);
    }

    #[test]
    fn number_range_is_empty_when_first_is_greater() {
        assert_eq!(matching("a{3..1}", &["a1", "a2", "a3"]), Vec::<&str>::new());
    }

    #[test]
    fn number_range_takes_no_number_beyond_i64() {
        let huge = format!("a{}", "9".repeat(30));
        assert_eq!(matching("a{1..9}", &[&huge]), Vec::<&str>::new());
        assert_eq!(matching("a{1..9}*", &[&huge]), [&huge]);
        assert_eq!(
            matching("a{1..9}", &["a18446744073709551621"]),
            Vec::<&str>::new()
        );
        assert_eq!(
            matching("{0..9223372036854775807}", &["-9223372036854775809"]),
            Vec::<&str>::new()
        );
    }

    #[test]
    fn number_range_takes_both_ends_of_i64() {
        let range = "{-9223372036854775808..9223372036854775807}";
        let paths = ["-9223372036854775808", "9223372036854775807"];
        assert_eq!(matching(range, &paths), paths);
    }

    #[test]
    fn backslash_escapes_next_character() {
        assert_eq!(matching(r"\*.nu", &["*.nu", "a.nu"]), ["*.nu"]);
        assert_eq!(matching(r"a\{b,c}", &["a{b,c}", "ab"]), ["a{b,c}"]);
        assert_eq!(matching(r"a\", &[r"a\", "a"]), [r"a\"]);
    }

    #[test]
    fn long_section_matches_only_path_it_spells() {
        let name = "a".repeat(100);
        let (shorter, longer) = (&name[1..], format!("{name}a"));
        let changed = format!("{}b{}", &name[..64], &name[65..]);
        let paths = [name.as_str(), shorter, &longer, &changed];
        assert_eq!(matching(&name, &paths), [name.as_str()]);
        assert_eq!(
            matching(&format!("{}*", &name[..70]), &paths),
            [name.as_str(), shorter, &longer]
        );
    }

    #[test]
    fn match_is_case_sensitive() {
        assert_eq!(matching("*.nu", &["a.NU"]), Vec::<&str>::new());
    }

    #[test]
    fn deeply_nested_braces_match() {
        let depth = 20_000;
        let section: String = iter::repeat_n("{a,", depth)
            .chain(iter::repeat_n("}", depth))
            .collect();
        assert_eq!(matching(&section, &["a", "b"]), ["a"]);
    }

    #[test]
    fn many_stars_fail_on_long_name_in_time() {
        let name = format!("{}.nu", "a".repeat(60));
        let section = format!("{}b", "*a".repeat(20));
        assert_eq!(matching(&section, &[&name]), Vec::<&str>::new());
    }

    #[test]
    fn many_empty_alternatives_fail_on_long_name_in_time() {
        let name = format!("{}.nu", "a".repeat(60));
        let section = format!("{}x", "{,}".repeat(40));
        assert_eq!(matching(&section, &[&name]), Vec::<&str>::new());
    }

    #[test]
    fn number_ranges_fail_on_long_digits_in_time() {
        let name = "1".repeat(250);
        let section = format!("{}x", "*{1..9}".repeat(50));
        assert_eq!(matching(&section, &[&name]), Vec::<&str>::new());
    }
}
