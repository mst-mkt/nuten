//! Functions that build each element.

use crate::{Element, GroupId, LineMode, TextWidth};

pub fn text(text: &str) -> Element<'_> {
    Element::Text {
        text,
        width: TextWidth::of(text),
    }
}

pub fn exact_text(text: &str) -> Element<'_> {
    Element::ExactText {
        text,
        width: TextWidth::of(text),
    }
}

pub fn space() -> Element<'static> {
    Element::Space
}

pub fn soft_line_break() -> Element<'static> {
    Element::Line(LineMode::Soft)
}

pub fn soft_line_break_or_space() -> Element<'static> {
    Element::Line(LineMode::SoftOrSpace)
}

pub fn hard_line_break() -> Element<'static> {
    empty_lines(0)
}

pub fn empty_line() -> Element<'static> {
    empty_lines(1)
}

pub fn empty_lines(blank_lines: u32) -> Element<'static> {
    Element::Line(LineMode::Hard {
        blank_lines,
    })
}

pub fn expand_parent() -> Element<'static> {
    Element::ExpandParent
}

pub fn line_suffix<'a>(elements: impl IntoIterator<Item = Element<'a>>) -> Element<'a> {
    Element::LineSuffix(elements.into_iter().collect())
}

pub fn line_suffix_boundary() -> Element<'static> {
    Element::LineSuffixBoundary
}

pub fn group<'a>(elements: impl IntoIterator<Item = Element<'a>>) -> Element<'a> {
    Element::Group {
        id: None,
        elements: elements.into_iter().collect(),
    }
}

pub fn group_with_id<'a>(
    id: GroupId,
    elements: impl IntoIterator<Item = Element<'a>>,
) -> Element<'a> {
    Element::Group {
        id: Some(id),
        elements: elements.into_iter().collect(),
    }
}

pub fn indent<'a>(elements: impl IntoIterator<Item = Element<'a>>) -> Element<'a> {
    Element::Indent {
        group: None,
        elements: elements.into_iter().collect(),
    }
}

pub fn indent_if_group_breaks<'a>(
    group: GroupId,
    elements: impl IntoIterator<Item = Element<'a>>,
) -> Element<'a> {
    Element::Indent {
        group: Some(group),
        elements: elements.into_iter().collect(),
    }
}

pub fn align<'a>(columns: u32, elements: impl IntoIterator<Item = Element<'a>>) -> Element<'a> {
    Element::Align {
        columns,
        group: None,
        elements: elements.into_iter().collect(),
    }
}

pub fn align_if_group_breaks<'a>(
    group: GroupId,
    columns: u32,
    elements: impl IntoIterator<Item = Element<'a>>,
) -> Element<'a> {
    Element::Align {
        columns,
        group: Some(group),
        elements: elements.into_iter().collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::{group, indent, soft_line_break_or_space, space, text};
    use crate::{Element, LineMode, TextWidth};

    #[test]
    fn text_carries_its_width() {
        assert_eq!(
            text("echo"),
            Element::Text {
                text: "echo",
                width: TextWidth::Width(4),
            }
        );
    }

    #[test]
    fn containers_nest_borrowed_and_static_elements() {
        let src = String::from("a");
        let doc = group(vec![
            space(),
            text(&src),
            indent(src.split(',').map(text).chain([soft_line_break_or_space()])),
        ]);
        assert_eq!(
            doc,
            Element::Group {
                id: None,
                elements: vec![
                    Element::Space,
                    text("a"),
                    Element::Indent {
                        group: None,
                        elements: vec![text("a"), Element::Line(LineMode::SoftOrSpace)],
                    },
                ],
            }
        );
    }
}
