//! The edit that turns one text into another.

use lsp_types::{Position, Range, TextEdit};

/// One edit that replaces the lines `old` and `new` do not share at their start and end.
/// Not a diff of characters, which costs more and splits edits the client may apply out of step.
pub(crate) fn between(old: &str, new: &str) -> Option<TextEdit> {
    if old == new {
        return None;
    }
    let old_lines = lines(old);
    let new_lines = lines(new);
    let shared_start = shared_count(old_lines.iter().copied(), new_lines.iter().copied());
    // Not into the shared start, since a line cannot be kept at both ends.
    let max_shared_end = old_lines.len().min(new_lines.len()) - shared_start;
    let shared_end = shared_count(
        old_lines.iter().rev().copied(),
        new_lines.iter().rev().copied(),
    )
    .min(max_shared_end);
    let replaced = &old_lines[..old_lines.len() - shared_end];
    Some(TextEdit {
        range: Range {
            start: Position::new(line_number(shared_start), 0),
            end: end_of(replaced),
        },
        new_text: new_lines[shared_start..new_lines.len() - shared_end].concat(),
    })
}

/// The lines of `text` as the protocol counts them, each with the break that ends it.
/// A `\r` alone breaks a line there, though nu reads it as part of the line.
fn lines(text: &str) -> Vec<&str> {
    let mut rest = text;
    std::iter::from_fn(|| {
        if rest.is_empty() {
            return None;
        }
        let end = rest
            .find(['\n', '\r'])
            .map_or(rest.len(), |at| at + break_len(&rest[at..]));
        let (line, after) = rest.split_at(end);
        rest = after;
        Some(line)
    })
    .collect()
}

fn break_len(text: &str) -> usize {
    if text.starts_with("\r\n") { 2 } else { 1 }
}

fn shared_count<'a>(
    old: impl Iterator<Item = &'a str>,
    new: impl Iterator<Item = &'a str>,
) -> usize {
    old.zip(new).take_while(|(old, new)| old == new).count()
}

/// The position after `lines`, which start a text.
fn end_of(lines: &[&str]) -> Position {
    match lines.last() {
        Some(last) if !last.ends_with(['\n', '\r']) => Position::new(
            line_number(lines.len() - 1),
            // In UTF-16 code units, as the protocol counts unless the client agrees otherwise.
            u32::try_from(last.encode_utf16().count())
                .expect("fewer UTF-16 units in a line than the protocol can count"),
        ),
        _ => Position::new(line_number(lines.len()), 0),
    }
}

fn line_number(count: usize) -> u32 {
    u32::try_from(count).expect("fewer lines than the protocol can number")
}

#[cfg(test)]
mod tests {
    use lsp_types::{Position, Range, TextEdit};

    use super::{between, lines};

    /// `text` with `edit` applied, as a client that counts in UTF-16 applies it.
    fn applied(text: &str, edit: &TextEdit) -> String {
        let offset = |position: Position| {
            let line_start: usize = lines(text)[..position.line as usize]
                .iter()
                .map(|line| line.len())
                .sum();
            let line = lines(text)
                .get(position.line as usize)
                .copied()
                .unwrap_or("");
            let column: usize = line
                .chars()
                .scan(0, |units, char| {
                    let before = *units;
                    *units += char.len_utf16();
                    Some((before, char.len_utf8()))
                })
                .take_while(|(before, _)| *before < position.character as usize)
                .map(|(_, len)| len)
                .sum();
            line_start + column
        };
        let (start, end) = (offset(edit.range.start), offset(edit.range.end));
        [&text[..start], edit.new_text.as_str(), &text[end..]].concat()
    }

    fn edit(start: (u32, u32), end: (u32, u32), new_text: &str) -> TextEdit {
        TextEdit {
            range: Range {
                start: Position::new(start.0, start.1),
                end: Position::new(end.0, end.1),
            },
            new_text: new_text.to_string(),
        }
    }

    #[test]
    fn same_texts_take_no_edit() {
        for text in ["", "ls\n", "ls"] {
            assert_eq!(between(text, text), None, "{text:?}");
        }
    }

    #[test]
    fn edit_replaces_only_lines_that_differ() {
        for (old, new, expected) in [
            ("a\nb \nc\n", "a\nb\nc\n", edit((1, 0), (2, 0), "b\n")),
            ("a\nc\n", "a\nb\nc\n", edit((1, 0), (1, 0), "b\n")),
            ("a\n\n\nb\n", "a\n\nb\n", edit((2, 0), (3, 0), "")),
            ("a \n", "a\n", edit((0, 0), (1, 0), "a\n")),
            ("", "a\n", edit((0, 0), (0, 0), "a\n")),
            ("a\n\n", "", edit((0, 0), (2, 0), "")),
        ] {
            assert_eq!(between(old, new), Some(expected), "{old:?} {new:?}");
        }
    }

    #[test]
    fn edit_ends_after_last_line_without_break_in_utf16_units() {
        for (old, new, expected) in [
            ("a\nls", "a\nls\n", edit((1, 0), (1, 2), "ls\n")),
            ("ls 日本", "ls 日本\n", edit((0, 0), (0, 5), "ls 日本\n")),
            ("ls 𝄞", "ls 𝄞\n", edit((0, 0), (0, 5), "ls 𝄞\n")),
        ] {
            assert_eq!(between(old, new), Some(expected), "{old:?} {new:?}");
        }
    }

    #[test]
    fn carriage_return_alone_breaks_line_as_protocol_counts() {
        for (old, new, expected) in [
            (
                "# a\rb\nls \n",
                "# a\rb\nls\n",
                edit((2, 0), (3, 0), "ls\n"),
            ),
            (
                "ls\r\nls \r\n",
                "ls\r\nls\r\n",
                edit((1, 0), (2, 0), "ls\r\n"),
            ),
            ("ls\r\n", "ls\n", edit((0, 0), (1, 0), "ls\n")),
            ("ls\r", "ls\r\n", edit((0, 0), (1, 0), "ls\r\n")),
        ] {
            assert_eq!(between(old, new), Some(expected), "{old:?} {new:?}");
        }
    }

    #[test]
    fn applying_edit_gives_new_text() {
        let texts = [
            "",
            "\n",
            "a",
            "a\n",
            "a\nb\n",
            "b\na\n",
            "a\n\nb",
            "a\r\nb\r\n",
            "a\rb\r",
            "a\r\r\n",
            "日本\n𝄞",
            "a\nb\na\n",
            "a\na\n",
        ];
        for old in texts {
            for new in texts {
                let text = between(old, new).map_or(old.to_string(), |edit| applied(old, &edit));
                assert_eq!(text, new, "{old:?} {new:?}");
            }
        }
    }
}
