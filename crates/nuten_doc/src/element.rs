//! The elements a document is made of.

use unicode_width::UnicodeWidthStr;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Document<'a> {
    pub elements: Vec<Element<'a>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Element<'a> {
    /// Printed as is, line breaks included. `width` is `TextWidth::of(text)`.
    Text {
        text: &'a str,
        width: TextWidth,
    },
    /// A space either way.
    Space,
    Line(LineMode),
    /// Breaks every group around it.
    ExpandParent,
    /// Held back until just before the next line break.
    LineSuffix(Vec<Element<'a>>),
    /// A line break here if a line suffix is held back.
    LineSuffixBoundary,
    /// Printed flat when it fits on the line, otherwise with its own `Line`s
    /// broken.
    Group(Vec<Element<'a>>),
    /// One level deeper after each line break inside.
    Indent(Vec<Element<'a>>),
}

/// What a `Line` prints, flat or broken.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineMode {
    /// Nothing, or a line break.
    Soft,
    /// A space, or a line break.
    SoftOrSpace,
    /// A line break either way.
    Hard,
    /// A line break and a blank line either way.
    Empty,
}

/// Columns a text takes on the line.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextWidth {
    Width(u32),
    /// The text has line breaks of its own, so it never fits a group on one
    /// line.
    Multiline,
}

impl TextWidth {
    /// Wide characters and emoji sequences take two columns; control
    /// characters, tabs included, take one.
    pub fn of(text: &str) -> TextWidth {
        if text.contains('\n') {
            return TextWidth::Multiline;
        }
        let width = if text.is_ascii() {
            text.len()
        } else {
            text.width()
        };
        TextWidth::Width(u32::try_from(width).unwrap_or(u32::MAX))
    }
}

#[cfg(test)]
mod tests {
    use super::TextWidth;

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
