use crate::{Document, Element, LineMode, TextWidth};

pub fn flat<'a>(document: &Document<'a>, elements: &[Element<'a>]) -> Option<Vec<Element<'a>>> {
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
        Document, exact_text, expand_parent, hard_line_break, line_suffix_boundary,
        soft_line_break, soft_line_break_or_space, space, text,
    };

    #[test]
    fn every_group_comes_out_flat() {
        let mut doc = Document::default();
        let indented = doc.indent([
            soft_line_break(),
            text("a"),
            soft_line_break_or_space(),
            text("b"),
        ]);
        let elements = [
            text("("),
            doc.group([indented, soft_line_break(), text(")"), expand_parent()]),
            doc.align(2, [space(), line_suffix_boundary()]),
            exact_text("'a'"),
            doc.if_break([hard_line_break()], [text(",")]),
        ];
        assert_eq!(
            flat(&doc, &elements),
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
        let mut doc = Document::default();
        let suffix = doc.line_suffix([text("# c")]);
        for element in [
            doc.group([text("a"), hard_line_break()]),
            text("a\nb"),
            exact_text("a\nb"),
            doc.indent([suffix]),
        ] {
            assert_eq!(flat(&doc, &[element]), None, "{element:?}");
        }
    }

    #[test]
    fn only_forced_break_at_any_depth_breaks_groups() {
        let mut doc = Document::default();
        for element in [
            doc.group([text("a"), hard_line_break()]),
            doc.indent([expand_parent()]),
            doc.align(2, [text("a\nb")]),
            doc.line_suffix([text("a\nb")]),
            text("a\nb"),
            exact_text("a\nb"),
            doc.if_break([], [hard_line_break()]),
        ] {
            assert!(breaks_groups(&doc, &[element]), "{element:?}");
        }
        for element in [
            doc.group([text("a"), soft_line_break_or_space(), text("b")]),
            doc.line_suffix([text("# c")]),
            doc.if_break([hard_line_break()], []),
        ] {
            assert!(!breaks_groups(&doc, &[element]), "{element:?}");
        }
    }
}
