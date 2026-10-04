use unicode_width::UnicodeWidthStr;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Element<'a> {
    /// `width` is `TextWidth::of(text)`.
    Text {
        text: &'a str,
        width: TextWidth,
    },
    /// `width` is `TextWidth::of(text)`.
    ExactText {
        text: &'a str,
        width: TextWidth,
    },
    Space,
    Line(LineMode),
    ExpandParent,
    LineSuffix(Box<[Element<'a>]>),
    LineSuffixBoundary,
    IfBreak {
        broken: Box<[Element<'a>]>,
        flat: Box<[Element<'a>]>,
    },
    Group {
        id: Option<GroupId>,
        elements: Box<[Element<'a>]>,
    },
    /// `group` must name one around the indent, or one printed before it.
    Indent {
        group: Option<GroupId>,
        elements: Box<[Element<'a>]>,
    },
    /// `group` must name one around the alignment, or one printed before it.
    Align {
        columns: u32,
        group: Option<GroupId>,
        elements: Box<[Element<'a>]>,
    },
}

/// A document must give each id to one group only.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct GroupId(pub(crate) u32);

#[derive(Debug, Default)]
pub struct GroupIds {
    next: u32,
}

impl GroupIds {
    pub fn fresh(&mut self) -> GroupId {
        let id = GroupId(self.next);
        self.next += 1;
        id
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineMode {
    Soft,
    SoftOrSpace,
    Hard { blank_lines: u32 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextWidth {
    Width(u32),
    Multiline,
}

impl Element<'_> {
    pub(crate) fn forces_break(&self) -> bool {
        matches!(
            self,
            Element::Line(LineMode::Hard { .. })
                | Element::ExpandParent
                | Element::Text {
                    width: TextWidth::Multiline,
                    ..
                }
                | Element::ExactText {
                    width: TextWidth::Multiline,
                    ..
                }
        )
    }
}

impl TextWidth {
    pub fn of(text: &str) -> TextWidth {
        if text.contains('\n') {
            TextWidth::Multiline
        } else {
            TextWidth::Width(columns(text))
        }
    }
}

/// `text` must have no line break.
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
    fn line_break_makes_text_multiline() {
        assert_eq!(TextWidth::of("a\nb"), TextWidth::Multiline);
        assert_eq!(TextWidth::of("a\r\nb"), TextWidth::Multiline);
        assert_eq!(TextWidth::of("a\rb"), TextWidth::Width(3));
    }
}
