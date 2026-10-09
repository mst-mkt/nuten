use std::num::NonZeroU32;

use unicode_width::UnicodeWidthStr;

use crate::{Children, Sides};

/// Takes 24 bytes where a plain layout takes 32, since a document is mostly its elements.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
    LineSuffix(Children),
    LineSuffixBoundary,
    IfBreak(Sides),
    Group {
        id: Option<GroupId>,
        elements: Children,
    },
    /// `group` must name one around the indent, or one printed before it.
    Indent {
        group: Option<GroupId>,
        elements: Children,
    },
    /// `group` must name one around the alignment, or one printed before it.
    Align {
        columns: u32,
        group: Option<GroupId>,
        elements: Children,
    },
}

/// A document must give each id to one group only.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct GroupId(NonZeroU32);

#[derive(Debug)]
pub struct GroupIds {
    next: NonZeroU32,
}

impl Default for GroupIds {
    fn default() -> Self {
        GroupIds {
            next: NonZeroU32::MIN,
        }
    }
}

impl GroupIds {
    pub fn fresh(&mut self) -> GroupId {
        let id = GroupId(self.next);
        self.next = self
            .next
            .checked_add(1)
            .expect("a document holds fewer than 2^32 groups");
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
    Width(Columns),
    Multiline,
}

// One more than the count, so that `Multiline` takes the zero.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Columns(NonZeroU32);

impl Columns {
    pub const fn new(columns: u32) -> Columns {
        Columns(NonZeroU32::MIN.saturating_add(columns))
    }

    pub const fn get(self) -> u32 {
        self.0.get() - 1
    }
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
            TextWidth::Width(Columns::new(columns(text)))
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
    use std::num::NonZeroU32;

    use super::{Columns, Element, GroupIds, TextWidth};

    fn width(columns: u32) -> TextWidth {
        TextWidth::Width(Columns::new(columns))
    }

    #[test]
    fn element_takes_24_bytes() {
        assert_eq!(size_of::<Element<'_>>(), 24);
    }

    #[test]
    fn fresh_ids_differ() {
        let mut ids = GroupIds::default();
        let first = ids.fresh();
        assert_ne!(first, ids.fresh());
    }

    #[test]
    #[should_panic(expected = "fewer than 2^32 groups")]
    fn fresh_ids_never_wrap_around() {
        let mut ids = GroupIds {
            next: NonZeroU32::MAX,
        };
        ids.fresh();
    }

    #[test]
    fn width_counts_columns() {
        assert_eq!(TextWidth::of(""), width(0));
        assert_eq!(TextWidth::of("echo"), width(4));
        assert_eq!(TextWidth::of("a\tb"), width(3));
        assert_eq!(TextWidth::of("日本語"), width(6));
        assert_eq!(TextWidth::of("café"), width(4));
        assert_eq!(TextWidth::of("e\u{301}"), width(1));
        assert_eq!(TextWidth::of("🦀"), width(2));
        assert_eq!(TextWidth::of("👍🏽"), width(2));
        assert_eq!(TextWidth::of("日\tb"), width(4));
    }

    #[test]
    fn line_break_makes_text_multiline() {
        assert_eq!(TextWidth::of("a\nb"), TextWidth::Multiline);
        assert_eq!(TextWidth::of("a\r\nb"), TextWidth::Multiline);
        assert_eq!(TextWidth::of("a\rb"), width(3));
    }

    #[test]
    fn columns_give_back_what_they_were_made_from() {
        for columns in [0, 1, 80, u32::MAX - 1] {
            assert_eq!(Columns::new(columns).get(), columns);
        }
    }

    #[test]
    fn columns_saturate_one_short_of_max() {
        assert_eq!(Columns::new(u32::MAX).get(), u32::MAX - 1);
    }
}
