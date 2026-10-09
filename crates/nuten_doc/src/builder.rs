use crate::{Children, Document, Element, GroupId, LineMode, TextWidth, flat};

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

/// Each container takes its children off the end of one list of what is written.
/// So no container makes a list of its own, only to copy it into the document and drop it.
#[derive(Debug)]
pub struct Builder<'a> {
    document: Document<'a>,
    written: Vec<Element<'a>>,
}

impl Default for Builder<'_> {
    fn default() -> Self {
        Builder {
            document: Document::new(),
            written: Vec::new(),
        }
    }
}

impl<'a> Builder<'a> {
    pub fn build(write: impl FnOnce(&mut Self)) -> Document<'a> {
        let mut builder = Builder::default();
        write(&mut builder);
        builder.finish()
    }

    pub fn finish(self) -> Document<'a> {
        self.document.finish(self.written)
    }
}

/// Each container hands its closure the implementor, not the bare builder.
/// So what is written inside a container can call the implementor's own methods.
pub trait Build<'a>: Sized {
    fn builder(&mut self) -> &mut Builder<'a>;

    fn push(&mut self, element: Element<'a>) {
        self.builder().written.push(element);
    }

    fn extend(&mut self, elements: impl IntoIterator<Item = Element<'a>>) {
        self.builder().written.extend(elements);
    }

    fn group(&mut self, write: impl FnOnce(&mut Self)) {
        wrap(self, write, |elements| Element::Group {
            id: None,
            elements,
        });
    }

    fn group_with_id(&mut self, id: GroupId, write: impl FnOnce(&mut Self)) {
        wrap(self, write, |elements| Element::Group {
            id: Some(id),
            elements,
        });
    }

    fn indent(&mut self, write: impl FnOnce(&mut Self)) {
        self.indent_levels(1, write);
    }

    fn indent_levels(&mut self, levels: u32, write: impl FnOnce(&mut Self)) {
        wrap_levels(self, levels, write, |elements| Element::Indent {
            group: None,
            elements,
        });
    }

    fn indent_if_group_breaks(&mut self, group: GroupId, write: impl FnOnce(&mut Self)) {
        self.indent_levels_if_group_breaks(group, 1, write);
    }

    fn indent_levels_if_group_breaks(
        &mut self,
        group: GroupId,
        levels: u32,
        write: impl FnOnce(&mut Self),
    ) {
        wrap_levels(self, levels, write, |elements| Element::Indent {
            group: Some(group),
            elements,
        });
    }

    fn align(&mut self, columns: u32, write: impl FnOnce(&mut Self)) {
        wrap(self, write, |elements| Element::Align {
            columns,
            group: None,
            elements,
        });
    }

    fn align_if_group_breaks(
        &mut self,
        group: GroupId,
        columns: u32,
        write: impl FnOnce(&mut Self),
    ) {
        wrap(self, write, |elements| Element::Align {
            columns,
            group: Some(group),
            elements,
        });
    }

    fn line_suffix(&mut self, write: impl FnOnce(&mut Self)) {
        wrap(self, write, Element::LineSuffix);
    }

    /// An empty side is kept, unlike an empty container, since it is what prints in that mode.
    fn if_break(&mut self, when_broken: impl FnOnce(&mut Self), when_flat: impl FnOnce(&mut Self)) {
        let broken = children_written_by(self, when_broken);
        let flat = children_written_by(self, when_flat);
        self.push(Element::IfBreak {
            broken,
            flat,
        });
    }

    fn flat_or(&mut self, write: impl FnOnce(&mut Self), otherwise: impl FnOnce(&mut Self)) {
        let start = self.builder().written.len();
        write(self);
        let builder = self.builder();
        let flattened = flat(&builder.document, &builder.written[start..]);
        builder.written.truncate(start);
        match flattened {
            Some(flattened) => builder.written.extend(flattened),
            None => otherwise(self),
        }
    }
}

impl<'a> Build<'a> for Builder<'a> {
    fn builder(&mut self) -> &mut Builder<'a> {
        self
    }
}

fn wrap<'a, B: Build<'a>>(
    this: &mut B,
    write: impl FnOnce(&mut B),
    container: impl Fn(Children) -> Element<'a>,
) {
    wrap_levels(this, 1, write, container);
}

/// An empty container is left out, since an empty group would still record whether it broke.
/// The elements that name the group would then follow a group that prints nothing.
fn wrap_levels<'a, B: Build<'a>>(
    this: &mut B,
    levels: u32,
    write: impl FnOnce(&mut B),
    container: impl Fn(Children) -> Element<'a>,
) {
    let start = this.builder().written.len();
    write(this);
    let builder = this.builder();
    if builder.written.len() == start {
        return;
    }
    for _ in 0..levels {
        let children = builder
            .document
            .push_children(builder.written.drain(start..));
        builder.written.push(container(children));
    }
}

fn children_written_by<'a, B: Build<'a>>(this: &mut B, write: impl FnOnce(&mut B)) -> Children {
    let start = this.builder().written.len();
    write(this);
    let builder = this.builder();
    builder
        .document
        .push_children(builder.written.drain(start..))
}

#[cfg(test)]
mod tests {
    use super::{Build, Builder, hard_line_break, soft_line_break_or_space, space, text};
    use crate::{Element, GroupId, LineMode, TextWidth};

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
        let doc = Builder::build(|b| {
            b.group(|b| {
                b.extend([space(), text(&src)]);
                b.indent(|b| b.extend([text(&src), soft_line_break_or_space()]));
            });
        });
        let [
            Element::Group {
                id: None,
                elements: group,
            },
        ] = doc.root()
        else {
            panic!("{:?} is no group", doc.root());
        };
        let [
            Element::Space,
            first,
            Element::Indent {
                group: None,
                elements: indent,
            },
        ] = doc.children(*group)
        else {
            panic!("the group holds what it was given");
        };
        assert_eq!(*first, text("a"));
        assert_eq!(
            doc.children(*indent),
            [text("a"), Element::Line(LineMode::SoftOrSpace)]
        );
    }

    #[test]
    fn empty_containers_are_left_out() {
        let doc = Builder::build(|b| {
            b.push(text("a"));
            b.group(|b| b.indent(|_| {}));
            b.group_with_id(GroupId(0), |_| {});
            b.indent_levels_if_group_breaks(GroupId(0), 2, |_| {});
        });
        assert_eq!(doc.root(), [text("a")]);
    }

    #[test]
    fn indent_levels_nest_one_indent_per_level() {
        let doc = Builder::build(|b| b.indent_levels(2, |b| b.push(text("a"))));
        let [
            Element::Indent {
                group: None,
                elements: outer,
            },
        ] = doc.root()
        else {
            panic!("{:?} is no indent", doc.root());
        };
        let [
            Element::Indent {
                group: None,
                elements: inner,
            },
        ] = doc.children(*outer)
        else {
            panic!("{:?} is no indent", doc.children(*outer));
        };
        assert_eq!(doc.children(*inner), [text("a")]);
    }

    #[test]
    fn zero_indent_levels_leave_children_in_place() {
        let doc = Builder::build(|b| b.indent_levels(0, |b| b.push(text("a"))));
        assert_eq!(doc.root(), [text("a")]);
    }

    #[test]
    fn if_break_keeps_both_sides_even_when_empty() {
        let doc = Builder::build(|b| b.if_break(|_| {}, |_| {}));
        let [
            Element::IfBreak {
                broken,
                flat,
            },
        ] = doc.root()
        else {
            panic!("{:?} is no if_break", doc.root());
        };
        assert_eq!(doc.children(*broken), []);
        assert_eq!(doc.children(*flat), []);
    }

    #[test]
    fn flat_or_writes_flat_form_after_what_came_before() {
        let doc = Builder::build(|b| {
            b.push(text("a"));
            b.flat_or(
                |b| b.group(|b| b.extend([soft_line_break_or_space(), text("b")])),
                |b| b.push(text("otherwise")),
            );
        });
        assert_eq!(doc.root(), [text("a"), space(), text("b")]);
    }

    #[test]
    fn flat_or_writes_otherwise_in_place_of_what_cannot_be_flat() {
        let doc = Builder::build(|b| {
            b.push(text("a"));
            b.flat_or(
                |b| b.group(|b| b.extend([text("b"), hard_line_break()])),
                |b| b.push(text("otherwise")),
            );
            b.push(text("c"));
        });
        assert_eq!(doc.root(), [text("a"), text("otherwise"), text("c")]);
    }
}
