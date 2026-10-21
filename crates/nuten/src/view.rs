use std::{iter, ops::Range, path::Path};

use crate::{
    diff::{Change, Difference, Hunk, changes},
    picture::pictured,
    style::{Style, Tone},
};

/// Bounds comparing within a change, which can cost the square of both sides.
const COMPARED_CHARS: usize = 10_000;

pub(crate) fn view(path: &Path, difference: &Difference<'_>, style: Style) -> String {
    // Lossy for names that are not UTF-8, as the list of `--list-different` is.
    let path = pictured(&path.display().to_string());
    let body = match &difference.hunks {
        Some(hunks) => hunks_body(&path, hunks, difference, style),
        None => endings_body(&path, difference, style),
    };
    body + &style.tone(Tone::Dim, "╰─") + "\n"
}

/// Not line by line, which would repeat one mark on every line.
fn endings_body(path: &str, difference: &Difference<'_>, style: Style) -> String {
    let note = format!(
        "line endings: {} → {}",
        endings(difference.old),
        endings(difference.new)
    );
    corner("╭─ ", path, style) + &edge(style) + &note + "\n"
}

fn hunks_body(path: &str, hunks: &[Hunk], difference: &Difference<'_>, style: Style) -> String {
    hunks
        .iter()
        .enumerate()
        .map(|(index, hunk)| {
            let mark = if index == 0 { "╭─ " } else { "├─ " };
            let line = hunk.changes[0].old.start + 1;
            corner(mark, &format!("{path}:{line}"), style)
                + &hunk_text(hunk, &difference.old_lines, &difference.new_lines, style)
        })
        .collect()
}

fn endings(text: &str) -> &'static str {
    let crlf = text.contains("\r\n");
    let lf = text
        .match_indices('\n')
        .any(|(at, _)| !text[..at].ends_with('\r'));
    match (crlf, lf) {
        (true, true) => "mixed",
        (true, false) => "CRLF",
        (false, _) => "LF",
    }
}

fn corner(mark: &str, place: &str, style: Style) -> String {
    style.tone(Tone::Dim, mark) + &style.bold(place) + "\n"
}

fn edge(style: Style) -> String {
    style.tone(Tone::Dim, "│ ")
}

fn hunk_text(hunk: &Hunk, old_lines: &[&str], new_lines: &[&str], style: Style) -> String {
    let kept_starts = [hunk.old.start]
        .into_iter()
        .chain(hunk.changes.iter().map(|change| change.old.end));
    let kept_ends = hunk
        .changes
        .iter()
        .map(|change| change.old.start)
        .chain([hunk.old.end]);
    let kept = kept_starts.zip(kept_ends).map(|(start, end)| {
        old_lines[start..end]
            .iter()
            .map(|line| line_text(line, iter::repeat(false), Tone::Dim, style))
            .collect::<String>()
    });
    let replaced = hunk
        .changes
        .iter()
        .map(|change| change_text(change, old_lines, new_lines, style))
        .chain([String::new()]);
    kept.zip(replaced)
        .flat_map(|(kept, replaced)| [kept, replaced])
        .collect()
}

/// Not each line against its counterpart, since formatting joins and splits lines.
fn change_text(change: &Change, old_lines: &[&str], new_lines: &[&str], style: Style) -> String {
    let old_lines = &old_lines[change.old.clone()];
    let new_lines = &new_lines[change.new.clone()];
    let old_chars: Vec<char> = old_lines.concat().chars().collect();
    let new_chars: Vec<char> = new_lines.concat().chars().collect();
    // Not within lines added or removed whole, whose signs tell all.
    let compared = !old_chars.is_empty()
        && !new_chars.is_empty()
        && old_chars.len() + new_chars.len() <= COMPARED_CHARS;
    let (old_ranges, new_ranges) = if compared {
        changed_chars(&old_chars, &new_chars)
    } else {
        (Vec::new(), Vec::new())
    };
    block_text(
        old_lines,
        &marks(old_chars.len(), &old_ranges),
        Tone::Removed,
        style,
    ) + &block_text(
        new_lines,
        &marks(new_chars.len(), &new_ranges),
        Tone::Added,
        style,
    )
}

/// By words and runs of spaces, not by characters, which would pair spaces that only moved.
fn changed_chars(old: &[char], new: &[char]) -> (Vec<Range<usize>>, Vec<Range<usize>>) {
    let old_tokens = tokens(old);
    let new_tokens = tokens(new);
    let texts = |chars: &[char], tokens: &[Range<usize>]| -> Vec<Vec<char>> {
        tokens
            .iter()
            .map(|token| chars[token.clone()].to_vec())
            .collect()
    };
    let within = changes(&texts(old, &old_tokens), &texts(new, &new_tokens));
    let chars_of = |tokens: &[Range<usize>], range: Range<usize>| match &tokens[range] {
        [first, .., last] => Some(first.start..last.end),
        [only] => Some(only.clone()),
        [] => None,
    };
    let old_ranges = within
        .iter()
        .filter_map(|change| chars_of(&old_tokens, change.old.clone()))
        .collect();
    let new_ranges = within
        .iter()
        .filter_map(|change| chars_of(&new_tokens, change.new.clone()))
        .collect();
    (old_ranges, new_ranges)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Class {
    Word,
    Space,
    Other,
}

fn tokens(chars: &[char]) -> Vec<Range<usize>> {
    let class = |ch: char| match ch {
        ch if ch.is_alphanumeric() || ch == '_' => Class::Word,
        // Alone, so that a line break a change adds or removes is marked apart from spaces.
        '\r' | '\n' => Class::Other,
        ch if ch.is_whitespace() => Class::Space,
        _ => Class::Other,
    };
    chars
        .chunk_by(|a, b| class(*a) == class(*b) && class(*a) != Class::Other)
        .scan(0, |start, run| {
            let token = *start..*start + run.len();
            *start = token.end;
            Some(token)
        })
        .collect()
}

/// `changed` is sorted and disjoint, as changes are.
fn marks(len: usize, changed: &[Range<usize>]) -> Vec<bool> {
    let kept_starts = [0].into_iter().chain(changed.iter().map(|range| range.end));
    let kept_ends = changed.iter().map(|range| range.start).chain([len]);
    let changed_lens = changed.iter().map(Range::len).chain([0]);
    kept_starts
        .zip(kept_ends)
        .zip(changed_lens)
        .flat_map(|((start, end), changed_len)| {
            iter::repeat_n(false, end - start).chain(iter::repeat_n(true, changed_len))
        })
        .collect()
}

fn block_text(lines: &[&str], marks: &[bool], tone: Tone, style: Style) -> String {
    let starts = lines.iter().scan(0, |next, line| {
        let start = *next;
        *next += line.chars().count();
        Some(start)
    });
    lines
        .iter()
        .zip(starts)
        .map(|(line, start)| line_text(line, marks[start..].iter().copied(), tone, style))
        .collect()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Weight {
    Normal,
    Strong,
    Faint,
}

fn line_text(line: &str, marks: impl Iterator<Item = bool>, tone: Tone, style: Style) -> String {
    // Not without color, where a picture for every space would look like code.
    let draws_spaces = style.colored() && tone != Tone::Dim;
    let runs = char_texts(line, marks, draws_spaces)
        .chunk_by(|a, b| a.1 == b.1)
        .map(|run| {
            let text: String = run.iter().map(|(text, _)| text.as_str()).collect();
            (text, run[0].1)
        })
        .filter(|(text, _)| !text.is_empty())
        .map(|(text, weight)| match weight {
            Weight::Normal => style.tone(tone, &text),
            Weight::Strong => style.strong(tone, &text),
            Weight::Faint => style.faint(tone, &text),
        })
        .collect::<String>();
    edge(style) + &gutter(tone, style) + &runs + "\n"
}

fn char_texts(
    line: &str,
    marks: impl Iterator<Item = bool>,
    draws_spaces: bool,
) -> Vec<(String, Weight)> {
    let break_len = if line.ends_with("\r\n") {
        2
    } else {
        usize::from(line.ends_with('\n'))
    };
    let break_start = line.chars().count() - break_len;
    line.chars()
        .zip(marks)
        .enumerate()
        .map(|(index, (ch, changed))| char_text(ch, changed, index >= break_start, draws_spaces))
        .collect()
}

/// A line break that stays is not drawn, since the view breaks the line there anyway.
/// One that changes is drawn even without color, as the lines would otherwise read the same.
fn char_text(ch: char, changed: bool, in_line_break: bool, draws_spaces: bool) -> (String, Weight) {
    match (changed, ch) {
        (false, _) if in_line_break => (String::new(), Weight::Normal),
        (true, '\n') => ("⏎".to_string(), Weight::Faint),
        (_, ' ') if draws_spaces => ("·".to_string(), Weight::Faint),
        (_, '\t') if draws_spaces => ("→".to_string(), Weight::Faint),
        (_, ch) if ch.is_whitespace() && draws_spaces => (char_picture(ch), Weight::Faint),
        (true, ch) => (char_picture(ch), Weight::Strong),
        (false, ch) => (char_picture(ch), Weight::Normal),
    }
}

fn char_picture(ch: char) -> String {
    pictured(ch.encode_utf8(&mut [0; 4]))
}

fn gutter(tone: Tone, style: Style) -> String {
    match (tone, style.colored()) {
        (Tone::Removed | Tone::Added, true) => style.tone(tone, "▍ "),
        (_, true) => "  ".to_string(),
        (Tone::Removed, false) => "-".to_string(),
        (Tone::Added, false) => "+".to_string(),
        (_, false) => " ".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::{COMPARED_CHARS, view};
    use crate::{diff::Difference, style::Style};

    fn plain(old: &str, new: &str) -> String {
        view(Path::new("a.nu"), &Difference::new(old, new), Style::PLAIN)
    }

    /// The colored view without its colors, with each run of bold characters in brackets.
    fn bracketed(old: &str, new: &str) -> String {
        let colored = view(
            Path::new("a.nu"),
            &Difference::new(old, new),
            Style::COLORED,
        )
        .replace("\u{1b}[1m\u{1b}[31m", "\u{1b}[Sm")
        .replace("\u{1b}[1m\u{1b}[32m", "\u{1b}[Sm");
        colored
            .split('\u{1b}')
            .map(|piece| piece.split_once('m').unwrap_or(("", piece)))
            .scan(false, |strong, (code, text)| {
                let closes = *strong && code == "[0";
                *strong = code == "[S" || (*strong && !closes);
                let open = if code == "[S" { "[" } else { "" };
                Some(format!("{}{open}{text}", if closes { "]" } else { "" }))
            })
            .collect()
    }

    fn lines_replaced_at(len: usize, replaced: &[(usize, &str)]) -> String {
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
    fn view_frames_change_under_place_of_its_first_line() {
        assert_eq!(
            plain(
                &lines_replaced_at(10, &[]),
                &lines_replaced_at(10, &[(5, "five")])
            ),
            joined(&[
                "╭─ a.nu:6",
                "│  2",
                "│  3",
                "│  4",
                "│ -5",
                "│ +five",
                "│  6",
                "│  7",
                "│  8",
                "╰─",
            ])
        );
    }

    #[test]
    fn view_starts_each_later_hunk_with_its_place() {
        assert_eq!(
            plain(
                &lines_replaced_at(12, &[]),
                &lines_replaced_at(12, &[(1, "one"), (10, "ten")])
            ),
            joined(&[
                "╭─ a.nu:2",
                "│  0",
                "│ -1",
                "│ +one",
                "│  2",
                "│  3",
                "│  4",
                "├─ a.nu:11",
                "│  7",
                "│  8",
                "│  9",
                "│ -10",
                "│ +ten",
                "│  11",
                "╰─",
            ])
        );
    }

    #[test]
    fn view_places_insertion_at_line_it_comes_before() {
        assert_eq!(
            plain("a\nb\n", "x\na\nb\n"),
            joined(&["╭─ a.nu:1", "│ +x", "│  a", "│  b", "╰─"])
        );
        assert_eq!(
            plain("a\nb\n", "a\nb\nx\n"),
            joined(&["╭─ a.nu:3", "│  a", "│  b", "│ +x", "╰─"])
        );
    }

    #[test]
    fn view_shows_deletion_alone_with_place_of_its_first_line() {
        assert_eq!(
            plain("a\nb\nc\n", "a\nc\n"),
            joined(&["╭─ a.nu:2", "│  a", "│ -b", "│  c", "╰─"])
        );
    }

    #[test]
    fn view_draws_tab_of_changed_line_as_arrow_without_marking_it() {
        assert_eq!(
            bracketed("\tls|x\n", "\tls | x\n"),
            joined(&["╭─ a.nu:1", "│ ▍ →ls|x", "│ ▍ →ls·|·x", "╰─"])
        );
    }

    #[test]
    fn view_marks_whole_word_that_changes() {
        assert_eq!(
            bracketed("ls foo_bar1\n", "ls foo_baz1\n"),
            joined(&["╭─ a.nu:1", "│ ▍ ls·[foo_bar1]", "│ ▍ ls·[foo_baz1]", "╰─"])
        );
    }

    #[test]
    fn view_marks_change_across_words_and_signs_as_one_run() {
        assert_eq!(
            bracketed("ls a.b\n", "ls x-y\n"),
            joined(&["╭─ a.nu:1", "│ ▍ ls·[a.b]", "│ ▍ ls·[x-y]", "╰─"])
        );
    }

    #[test]
    fn view_marks_characters_of_later_lines_of_change_after_multibyte_characters() {
        assert_eq!(
            bracketed("é\nls|x\n", "é!\nls|y\n"),
            joined(&[
                "╭─ a.nu:1",
                "│ ▍ é",
                "│ ▍ ls|[x]",
                "│ ▍ é[!]",
                "│ ▍ ls|[y]",
                "╰─"
            ])
        );
    }

    #[test]
    fn view_compares_change_by_words_and_runs_of_spaces() {
        assert_eq!(
            bracketed("if x {\n\tls   \n}\n", "if x {\n    ls\n}\n"),
            joined(&[
                "╭─ a.nu:2",
                "│   if x {",
                "│ ▍ →ls···",
                "│ ▍ ····ls",
                "│   }",
                "╰─"
            ])
        );
    }

    #[test]
    fn view_leaves_spaces_as_they_are_without_color() {
        assert_eq!(
            plain("if x {\n\tls  a  \n}\n", "if x {\n    ls  a\n}\n"),
            joined(&[
                "╭─ a.nu:2",
                "│  if x {",
                "│ -\tls  a  ",
                "│ +    ls  a",
                "│  }",
                "╰─"
            ])
        );
    }

    #[test]
    fn view_marks_line_break_it_adds_or_removes() {
        assert_eq!(
            plain("ls", "ls\n"),
            joined(&["╭─ a.nu:1", "│ -ls", "│ +ls⏎", "╰─"])
        );
        assert_eq!(
            plain("ls\n", "ls"),
            joined(&["╭─ a.nu:1", "│ -ls⏎", "│ +ls", "╰─"])
        );
    }

    #[test]
    fn view_hides_carriage_return_of_line_ending_that_stays() {
        assert_eq!(
            plain("a\r\nls|x\r\n", "a\r\nls | x\r\n"),
            joined(&["╭─ a.nu:2", "│  a", "│ -ls|x", "│ +ls | x", "╰─"])
        );
    }

    #[test]
    fn view_shows_carriage_return_that_ends_no_line() {
        assert_eq!(
            plain("print \"a\r\"", "print \"a\r\"\n"),
            joined(&["╭─ a.nu:1", "│ -print \"a␍\"", "│ +print \"a␍\"⏎", "╰─"])
        );
    }

    #[test]
    fn view_compares_change_as_whole_across_lines_it_joins() {
        assert_eq!(
            plain("ls\n| first\n", "ls | first\n"),
            joined(&["╭─ a.nu:1", "│ -ls⏎", "│ -| first", "│ +ls | first", "╰─"])
        );
    }

    #[test]
    fn view_marks_nothing_within_change_too_long_to_compare() {
        let old = format!("[{}]\n", "1,".repeat(COMPARED_CHARS));
        let new = format!("[{}]\n", "1, ".repeat(COMPARED_CHARS));
        let text = plain(&old, &new);
        assert!(!text.contains('·'));
        assert!(text.contains("\n│ +[1, 1, "));
    }

    #[test]
    fn view_shows_control_characters_as_pictures() {
        assert_eq!(
            plain(
                "print \"\u{1b}[0m\"\nls|\u{7f}\n",
                "print \"\u{1b}[0m\"\nls | \u{7f}\n"
            ),
            joined(&[
                "╭─ a.nu:2",
                "│  print \"␛[0m\"",
                "│ -ls|␡",
                "│ +ls | ␡",
                "╰─",
            ])
        );
    }

    #[test]
    fn view_draws_other_space_of_changed_line_faint_by_its_picture() {
        let text = view(
            Path::new("a.nu"),
            &Difference::new("ls|x\u{b}\n", "ls | x\u{b}\n"),
            Style::COLORED,
        );
        assert!(text.contains("\u{1b}[2m\u{1b}[31m␋\u{1b}[0m"), "{text:?}");
        assert!(text.contains("\u{1b}[2m\u{1b}[32m␋\u{1b}[0m"), "{text:?}");
    }

    #[test]
    fn view_shows_control_characters_of_path_as_pictures() {
        let text = view(
            Path::new("a\u{1b}.nu"),
            &Difference::new("ls|x\n", "ls | x\n"),
            Style::PLAIN,
        );
        assert!(text.starts_with("╭─ a␛.nu:1\n"), "{text}");
    }

    #[test]
    fn view_sums_up_change_of_line_endings_alone() {
        assert_eq!(
            plain("a\r\nb\r\n", "a\nb\n"),
            joined(&["╭─ a.nu", "│ line endings: CRLF → LF", "╰─"])
        );
        assert_eq!(
            plain("a\nb\n", "a\r\nb\r\n"),
            joined(&["╭─ a.nu", "│ line endings: LF → CRLF", "╰─"])
        );
        assert_eq!(
            plain("a\nb\r\n", "a\r\nb\r\n"),
            joined(&["╭─ a.nu", "│ line endings: mixed → CRLF", "╰─"])
        );
    }

    #[test]
    fn view_colors_changes_and_draws_every_space_of_changed_lines_faint() {
        let dim = |text: &str| format!("\u{1b}[2m{text}\u{1b}[0m");
        let bold = |text: &str| format!("\u{1b}[1m{text}\u{1b}[0m");
        let removed = |text: &str| format!("\u{1b}[31m{text}\u{1b}[0m");
        let added = |text: &str| format!("\u{1b}[32m{text}\u{1b}[0m");
        let strong_removed = |text: &str| format!("\u{1b}[1m\u{1b}[31m{text}\u{1b}[0m");
        let strong_added = |text: &str| format!("\u{1b}[1m\u{1b}[32m{text}\u{1b}[0m");
        let faint_removed = |text: &str| format!("\u{1b}[2m\u{1b}[31m{text}\u{1b}[0m");
        let faint_added = |text: &str| format!("\u{1b}[2m\u{1b}[32m{text}\u{1b}[0m");
        let lines = [
            dim("╭─ ") + &bold("a.nu:2"),
            dim("│ ") + "  " + &dim("ls x"),
            dim("│ ")
                + &removed("▍ ")
                + &removed("ls")
                + &faint_removed("·")
                + &removed("|")
                + &strong_removed("x"),
            dim("│ ")
                + &added("▍ ")
                + &added("ls")
                + &faint_added("·")
                + &added("|")
                + &faint_added("·")
                + &strong_added("y")
                + &faint_added("⏎"),
            dim("╰─"),
        ];
        assert_eq!(
            view(
                Path::new("a.nu"),
                &Difference::new("ls x\nls |x", "ls x\nls | y\n"),
                Style::COLORED
            ),
            lines.map(|line| line + "\n").concat()
        );
    }
}
