//! A document laid out on one line whatever its width, and what keeps it off one line.

use crate::{Element, LineMode, TextWidth};

/// `elements` as the printer prints them with every group flat, whether or not they fit.
/// `None` where a hard line break, text over lines, or a line suffix keeps them off one line.
pub fn flat<'a>(elements: &[Element<'a>]) -> Option<Vec<Element<'a>>> {
    elements
        .iter()
        .map(flat_element)
        .collect::<Option<Vec<_>>>()
        .map(|elements| elements.into_iter().flatten().collect())
}

/// One inside a `LineSuffix` counts too, since `fits` measures the suffix with the line.
pub fn breaks_groups(elements: &[Element<'_>]) -> bool {
    elements.iter().any(|element| {
        element.forces_break()
            || match element {
                Element::Group {
                    elements, ..
                }
                | Element::Indent(elements)
                | Element::Align {
                    elements, ..
                }
                | Element::LineSuffix(elements) => breaks_groups(elements),
                _ => false,
            }
    })
}

fn flat_element<'a>(element: &Element<'a>) -> Option<Vec<Element<'a>>> {
    match element {
        Element::Text {
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
        | Element::Space
        | Element::LineSuffixBoundary => Some(vec![element.clone()]),
        Element::Line(LineMode::Soft) | Element::ExpandParent => Some(Vec::new()),
        Element::Line(LineMode::SoftOrSpace) => Some(vec![Element::Space]),
        Element::Group {
            elements, ..
        }
        | Element::Indent(elements)
        | Element::Align {
            elements, ..
        } => flat(elements),
    }
}

#[cfg(test)]
mod tests {
    use super::{breaks_groups, flat};
    use crate::{
        align, expand_parent, group, hard_line_break, indent, line_suffix, line_suffix_boundary,
        soft_line_break, soft_line_break_or_space, space, text,
    };

    #[test]
    fn every_group_comes_out_flat() {
        let doc = [
            text("("),
            group([
                indent([
                    soft_line_break(),
                    text("a"),
                    soft_line_break_or_space(),
                    text("b"),
                ]),
                soft_line_break(),
                text(")"),
                expand_parent(),
            ]),
            align(2, [space(), line_suffix_boundary()]),
        ];
        assert_eq!(
            flat(&doc),
            Some(vec![
                text("("),
                text("a"),
                space(),
                text("b"),
                text(")"),
                space(),
                line_suffix_boundary(),
            ])
        );
    }

    #[test]
    fn hard_line_breaks_and_line_suffixes_prevent_flat() {
        for doc in [
            vec![group([text("a"), hard_line_break()])],
            vec![text("a\nb")],
            vec![indent([line_suffix([text("# c")])])],
        ] {
            assert_eq!(flat(&doc), None, "{doc:?}");
        }
    }

    #[test]
    fn only_forced_break_at_any_depth_breaks_groups() {
        for doc in [
            vec![group([text("a"), hard_line_break()])],
            vec![indent([expand_parent()])],
            vec![align(2, [text("a\nb")])],
            vec![line_suffix([text("a\nb")])],
            vec![text("a\nb")],
        ] {
            assert!(breaks_groups(&doc), "{doc:?}");
        }
        for doc in [
            vec![group([text("a"), soft_line_break_or_space(), text("b")])],
            vec![line_suffix([text("# c")])],
        ] {
            assert!(!breaks_groups(&doc), "{doc:?}");
        }
    }
}
