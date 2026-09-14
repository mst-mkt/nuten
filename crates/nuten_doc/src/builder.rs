//! Functions that build each element.

use crate::{Element, LineMode, TextWidth};

pub fn text(text: &str) -> Element<'_> {
    Element::Text {
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
    Element::Line(LineMode::Hard)
}

pub fn empty_line() -> Element<'static> {
    Element::Line(LineMode::Empty)
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
    Element::Group(elements.into_iter().collect())
}

pub fn indent<'a>(elements: impl IntoIterator<Item = Element<'a>>) -> Element<'a> {
    Element::Indent(elements.into_iter().collect())
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
            Element::Group(vec![
                Element::Space,
                text("a"),
                Element::Indent(vec![text("a"), Element::Line(LineMode::SoftOrSpace)]),
            ])
        );
    }
}
