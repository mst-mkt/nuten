use crate::{Document, Element, GroupId, LineMode, TextWidth};

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

pub fn line_suffix_boundary() -> Element<'static> {
    Element::LineSuffixBoundary
}

impl<'a> Document<'a> {
    pub fn line_suffix(&mut self, elements: impl IntoIterator<Item = Element<'a>>) -> Element<'a> {
        Element::LineSuffix(self.push_children(elements))
    }

    pub fn if_break(
        &mut self,
        broken: impl IntoIterator<Item = Element<'a>>,
        flat: impl IntoIterator<Item = Element<'a>>,
    ) -> Element<'a> {
        Element::IfBreak {
            broken: self.push_children(broken),
            flat: self.push_children(flat),
        }
    }

    pub fn group(&mut self, elements: impl IntoIterator<Item = Element<'a>>) -> Element<'a> {
        Element::Group {
            id: None,
            elements: self.push_children(elements),
        }
    }

    pub fn group_with_id(
        &mut self,
        id: GroupId,
        elements: impl IntoIterator<Item = Element<'a>>,
    ) -> Element<'a> {
        Element::Group {
            id: Some(id),
            elements: self.push_children(elements),
        }
    }

    pub fn indent(&mut self, elements: impl IntoIterator<Item = Element<'a>>) -> Element<'a> {
        Element::Indent {
            group: None,
            elements: self.push_children(elements),
        }
    }

    pub fn indent_if_group_breaks(
        &mut self,
        group: GroupId,
        elements: impl IntoIterator<Item = Element<'a>>,
    ) -> Element<'a> {
        Element::Indent {
            group: Some(group),
            elements: self.push_children(elements),
        }
    }

    pub fn align(
        &mut self,
        columns: u32,
        elements: impl IntoIterator<Item = Element<'a>>,
    ) -> Element<'a> {
        Element::Align {
            columns,
            group: None,
            elements: self.push_children(elements),
        }
    }

    pub fn align_if_group_breaks(
        &mut self,
        group: GroupId,
        columns: u32,
        elements: impl IntoIterator<Item = Element<'a>>,
    ) -> Element<'a> {
        Element::Align {
            columns,
            group: Some(group),
            elements: self.push_children(elements),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{soft_line_break_or_space, space, text};
    use crate::{Document, Element, LineMode, TextWidth};

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
        let mut doc = Document::default();
        let indent = doc.indent(src.split(',').map(text).chain([soft_line_break_or_space()]));
        let group = doc.group(vec![space(), text(&src), indent]);
        let Element::Group {
            id: None,
            elements,
        } = group
        else {
            panic!("{group:?} is no group");
        };
        assert_eq!(doc.children(elements), [Element::Space, text("a"), indent]);
        let Element::Indent {
            group: None,
            elements,
        } = indent
        else {
            panic!("{indent:?} is no indent");
        };
        assert_eq!(
            doc.children(elements),
            [text("a"), Element::Line(LineMode::SoftOrSpace)]
        );
    }
}
