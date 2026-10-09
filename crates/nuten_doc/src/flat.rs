use crate::{Document, Element, LineMode, TextWidth};

pub(crate) fn flat<'a>(
    document: &Document<'a>,
    elements: &[Element<'a>],
) -> Option<Vec<Element<'a>>> {
    flat_onto(document, Vec::with_capacity(elements.len()), elements)
}

/// A forced break inside a `LineSuffix` counts too, since `fits` measures the suffix with the line.
/// One on the broken side of an `IfBreak` does not, since that side prints only after a break.
pub fn breaks_groups(document: &Document<'_>, elements: &[Element<'_>]) -> bool {
    elements.iter().any(|element| {
        element.forces_break()
            || match element {
                Element::Group {
                    elements, ..
                }
                | Element::Indent {
                    elements, ..
                }
                | Element::Align {
                    elements, ..
                }
                | Element::IfBreak {
                    flat: elements, ..
                }
                | Element::LineSuffix(elements) => {
                    breaks_groups(document, document.children(*elements))
                },
                _ => false,
            }
    })
}

fn flat_onto<'a>(
    document: &Document<'a>,
    flattened: Vec<Element<'a>>,
    elements: &[Element<'a>],
) -> Option<Vec<Element<'a>>> {
    elements.iter().try_fold(flattened, |flattened, element| {
        flat_element_onto(document, flattened, element)
    })
}

fn flat_element_onto<'a>(
    document: &Document<'a>,
    flattened: Vec<Element<'a>>,
    element: &Element<'a>,
) -> Option<Vec<Element<'a>>> {
    match element {
        Element::Text {
            width: TextWidth::Multiline,
            ..
        }
        | Element::ExactText {
            width: TextWidth::Multiline,
            ..
        }
        | Element::Line(LineMode::Hard {
            ..
        })
        | Element::LineSuffix(_) => None,
        Element::Text {
            ..
        }
        | Element::ExactText {
            ..
        }
        | Element::Space
        | Element::LineSuffixBoundary => Some(pushed(flattened, *element)),
        Element::Line(LineMode::Soft) | Element::ExpandParent => Some(flattened),
        Element::Line(LineMode::SoftOrSpace) => Some(pushed(flattened, Element::Space)),
        Element::Group {
            elements, ..
        }
        | Element::Indent {
            elements, ..
        }
        | Element::Align {
            elements, ..
        }
        | Element::IfBreak {
            flat: elements, ..
        } => flat_onto(document, flattened, document.children(*elements)),
    }
}

fn pushed<'a>(mut elements: Vec<Element<'a>>, element: Element<'a>) -> Vec<Element<'a>> {
    elements.push(element);
    elements
}

#[cfg(test)]
mod tests {
    use super::{breaks_groups, flat};
    use crate::{
        Build, Builder, Document, Element, exact_text, expand_parent, hard_line_break,
        line_suffix_boundary, soft_line_break, soft_line_break_or_space, space, text,
    };

    fn flat_root<'a>(doc: &Document<'a>) -> Option<Vec<Element<'a>>> {
        flat(doc, doc.root())
    }

    #[test]
    fn every_group_comes_out_flat() {
        let doc = Builder::build(|b| {
            b.push(text("("));
            b.group(|b| {
                b.indent(|b| {
                    b.extend([
                        soft_line_break(),
                        text("a"),
                        soft_line_break_or_space(),
                        text("b"),
                    ]);
                });
                b.extend([soft_line_break(), text(")"), expand_parent()]);
            });
            b.align(2, |b| b.extend([space(), line_suffix_boundary()]));
            b.push(exact_text("'a'"));
            b.if_break(|b| b.push(hard_line_break()), |b| b.push(text(",")));
        });
        assert_eq!(
            flat_root(&doc),
            Some(vec![
                text("("),
                text("a"),
                space(),
                text("b"),
                text(")"),
                space(),
                line_suffix_boundary(),
                exact_text("'a'"),
                text(","),
            ])
        );
    }

    #[test]
    fn hard_line_breaks_and_line_suffixes_prevent_flat() {
        let docs = [
            Builder::build(|b| b.group(|b| b.extend([text("a"), hard_line_break()]))),
            Builder::build(|b| b.push(text("a\nb"))),
            Builder::build(|b| b.push(exact_text("a\nb"))),
            Builder::build(|b| b.indent(|b| b.line_suffix(|b| b.push(text("# c"))))),
        ];
        for doc in docs {
            assert_eq!(flat_root(&doc), None, "{doc:?}");
        }
    }

    #[test]
    fn only_forced_break_at_any_depth_breaks_groups() {
        let breaking = [
            Builder::build(|b| b.group(|b| b.extend([text("a"), hard_line_break()]))),
            Builder::build(|b| b.indent(|b| b.push(expand_parent()))),
            Builder::build(|b| b.align(2, |b| b.push(text("a\nb")))),
            Builder::build(|b| b.line_suffix(|b| b.push(text("a\nb")))),
            Builder::build(|b| b.push(text("a\nb"))),
            Builder::build(|b| b.push(exact_text("a\nb"))),
            Builder::build(|b| b.if_break(|_| {}, |b| b.push(hard_line_break()))),
        ];
        for doc in breaking {
            assert!(breaks_groups(&doc, doc.root()), "{doc:?}");
        }
        let not_breaking = [
            Builder::build(|b| {
                b.group(|b| b.extend([text("a"), soft_line_break_or_space(), text("b")]));
            }),
            Builder::build(|b| b.line_suffix(|b| b.push(text("# c")))),
            Builder::build(|b| b.if_break(|b| b.push(hard_line_break()), |_| {})),
        ];
        for doc in not_breaking {
            assert!(!breaks_groups(&doc, doc.root()), "{doc:?}");
        }
    }
}
