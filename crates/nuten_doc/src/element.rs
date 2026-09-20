//! The elements a document is made of.

use unicode_width::UnicodeWidthStr;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Element<'a> {
    /// Printed as is, but for its line breaks, which come out as the printer's line ending.
    /// `width` is `TextWidth::of(text)`.
    Text {
        text: &'a str,
        width: TextWidth,
    },
    /// A space either way.
    Space,
    Line(LineMode),
    /// Breaks every group around it.
    ExpandParent,
    /// Held back until just before the next line break, or the end of the output.
    /// Printed then with the indent and mode of its own place.
    LineSuffix(Vec<Element<'a>>),
    /// A hard line break here if a line suffix is held back.
    LineSuffixBoundary,
    /// Printed flat when it fits on the line, otherwise with its own `Line`s broken.
    /// An `Align` after it or inside it can depend on that through `id`.
    Group {
        id: Option<GroupId>,
        elements: Vec<Element<'a>>,
    },
    /// One level deeper after each line break inside.
    Indent(Vec<Element<'a>>),
    /// `columns` further, in spaces, after each line break inside.
    /// Only when `group` is `None`, or names a group that broke.
    /// That group must be one around the alignment, or one printed before it.
    /// Zero columns are no alignment.
    /// With tabs, an alignment that a deeper level follows takes a tab instead.
    /// No space then comes before a tab.
    Align {
        columns: u32,
        group: Option<GroupId>,
        elements: Vec<Element<'a>>,
    },
}

/// Names a group; the document keeps each id to one group.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct GroupId(pub(crate) u32);

/// Gives out the ids of one document, each once.
#[derive(Debug, Default)]
pub struct GroupIds {
    next: u32,
}

impl GroupIds {
    /// An id no group has yet.
    pub fn fresh(&mut self) -> GroupId {
        let id = GroupId(self.next);
        self.next += 1;
        id
    }
}

/// What a `Line` prints, flat or broken.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineMode {
    /// Nothing, or a line break.
    Soft,
    /// A space, or a line break.
    SoftOrSpace,
    /// A line break and `blank_lines` blank lines either way, less what is there already.
    Hard { blank_lines: u32 },
}

/// Columns a text takes on the line.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextWidth {
    Width(u32),
    /// The text has line breaks of its own, so it never fits a group on one line.
    Multiline,
}

impl TextWidth {
    /// Wide characters and emoji sequences take two columns.
    /// Control characters, tabs included, take one.
    /// A line break is a `\n`, with or without a `\r` before it.
    /// A `\r` alone is an ordinary character.
    pub fn of(text: &str) -> TextWidth {
        if text.contains('\n') {
            TextWidth::Multiline
        } else {
            TextWidth::Width(columns(text))
        }
    }
}

/// Columns `text` takes on one line; `text` has no line break.
pub(crate) fn columns(text: &str) -> u32 {
    let width = if text.is_ascii() {
        text.len()
    } else {
        text.width()
    };
    u32::try_from(width).unwrap_or(u32::MAX)
}

#[cfg(test)]
mod tests {
    use super::{GroupIds, TextWidth};

    #[test]
    fn fresh_ids_differ() {
        let mut ids = GroupIds::default();
        let first = ids.fresh();
        assert_ne!(first, ids.fresh());
    }

    #[test]
    fn width_counts_columns() {
        assert_eq!(TextWidth::of(""), TextWidth::Width(0));
        assert_eq!(TextWidth::of("echo"), TextWidth::Width(4));
        assert_eq!(TextWidth::of("a\tb"), TextWidth::Width(3));
        assert_eq!(TextWidth::of("日本語"), TextWidth::Width(6));
        assert_eq!(TextWidth::of("café"), TextWidth::Width(4));
        assert_eq!(TextWidth::of("e\u{301}"), TextWidth::Width(1));
        assert_eq!(TextWidth::of("🦀"), TextWidth::Width(2));
        assert_eq!(TextWidth::of("👍🏽"), TextWidth::Width(2));
        assert_eq!(TextWidth::of("日\tb"), TextWidth::Width(4));
    }

    #[test]
    fn a_line_break_makes_the_text_multiline() {
        assert_eq!(TextWidth::of("a\nb"), TextWidth::Multiline);
        assert_eq!(TextWidth::of("a\r\nb"), TextWidth::Multiline);
        assert_eq!(TextWidth::of("a\rb"), TextWidth::Width(3));
    }
}
