//! Prints a document as text.

use std::iter;

use crate::{Document, Element, LineMode};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrinterOptions {
    /// Columns a line may take before groups break.
    pub line_width: u32,
    pub indent_style: IndentStyle,
    /// Columns per indent level, and per tab when indenting with tabs.
    pub indent_width: u8,
    pub line_ending: LineEnding,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IndentStyle {
    Tab,
    Space,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineEnding {
    Lf,
    CrLf,
}

impl LineEnding {
    const fn as_str(self) -> &'static str {
        match self {
            LineEnding::Lf => "\n",
            LineEnding::CrLf => "\r\n",
        }
    }
}

pub fn print(document: &Document<'_>, options: &PrinterOptions) -> String {
    let mut printer = Printer {
        options,
        stack: document
            .elements
            .iter()
            .rev()
            .map(|element| (0, element))
            .collect(),
        out: String::new(),
        pending_indent: None,
    };
    printer.print_all();
    printer.out
}

/// The indent of a line goes out with its first text or space, so an empty
/// line stays empty.
struct Printer<'a> {
    options: &'a PrinterOptions,
    /// What is left to print, the next element on top, with its indent level.
    stack: Vec<(u32, &'a Element<'a>)>,
    out: String,
    pending_indent: Option<u32>,
}

impl<'a> Printer<'a> {
    fn print_all(&mut self) {
        while let Some((indent, element)) = self.stack.pop() {
            match element {
                Element::Text {
                    text, ..
                } => self.push_text(text),
                Element::Space => self.push_text(" "),
                Element::Line(LineMode::Empty) => {
                    self.push_line_break(indent);
                    self.push_line_break(indent);
                },
                Element::Line(_) => self.push_line_break(indent),
                Element::ExpandParent | Element::LineSuffixBoundary => {},
                Element::LineSuffix(elements) | Element::Group(elements) => {
                    self.push_elements(indent, elements);
                },
                Element::Indent(elements) => {
                    self.push_elements(indent + 1, elements);
                },
            }
        }
    }

    fn push_elements(&mut self, indent: u32, elements: &'a [Element<'a>]) {
        self.stack
            .extend(elements.iter().rev().map(|element| (indent, element)));
    }

    fn push_text(&mut self, text: &str) {
        if let Some(indent) = self.pending_indent.take() {
            let (unit, count) = match self.options.indent_style {
                IndentStyle::Tab => ('\t', indent),
                IndentStyle::Space => (' ', indent * u32::from(self.options.indent_width)),
            };
            self.out.extend(iter::repeat_n(unit, count as usize));
        }
        self.out.push_str(text);
    }

    fn push_line_break(&mut self, indent: u32) {
        self.out.push_str(self.options.line_ending.as_str());
        self.pending_indent = Some(indent);
    }
}

#[cfg(test)]
mod tests {
    use super::{IndentStyle, LineEnding, PrinterOptions, print};
    use crate::{Document, Element, empty_line, hard_line_break, indent, space, text};

    fn printed(
        elements: Vec<Element<'_>>,
        indent_style: IndentStyle,
        line_ending: LineEnding,
    ) -> String {
        let options = PrinterOptions {
            line_width: 80,
            indent_style,
            indent_width: 2,
            line_ending,
        };
        print(
            &Document {
                elements,
            },
            &options,
        )
    }

    #[test]
    fn text_and_space_go_out_as_they_are() {
        let doc = vec![text("echo"), space(), text("a")];
        assert_eq!(printed(doc, IndentStyle::Space, LineEnding::Lf), "echo a");
    }

    #[test]
    fn hard_line_breaks_indent_the_next_text() {
        let doc = vec![
            text("a"),
            indent(vec![
                hard_line_break(),
                text("b"),
                indent(vec![hard_line_break(), text("c")]),
            ]),
            hard_line_break(),
            text("d"),
        ];
        assert_eq!(
            printed(doc.clone(), IndentStyle::Space, LineEnding::Lf),
            "a\n  b\n    c\nd"
        );
        assert_eq!(
            printed(doc, IndentStyle::Tab, LineEnding::Lf),
            "a\n\tb\n\t\tc\nd"
        );
    }

    #[test]
    fn indent_without_a_line_break_prints_nothing() {
        let doc = vec![indent(vec![text("a")])];
        assert_eq!(printed(doc, IndentStyle::Space, LineEnding::Lf), "a");
    }

    #[test]
    fn a_space_at_the_start_of_a_line_comes_after_the_indent() {
        let doc = vec![indent(vec![hard_line_break(), space(), text("b")])];
        assert_eq!(printed(doc, IndentStyle::Space, LineEnding::Lf), "\n   b");
    }

    #[test]
    fn empty_lines_carry_no_indent() {
        let doc = vec![text("a"), indent(vec![empty_line(), text("b")])];
        assert_eq!(printed(doc, IndentStyle::Space, LineEnding::Lf), "a\n\n  b");
    }

    #[test]
    fn a_final_line_break_leaves_no_indent_behind() {
        let doc = vec![indent(vec![text("a"), hard_line_break()])];
        assert_eq!(printed(doc, IndentStyle::Space, LineEnding::Lf), "a\n");
    }

    #[test]
    fn line_ending_is_configurable() {
        let doc = vec![text("a"), indent(vec![hard_line_break(), text("b")])];
        assert_eq!(
            printed(doc, IndentStyle::Space, LineEnding::CrLf),
            "a\r\n  b"
        );
    }

    #[test]
    fn indent_width_sets_the_spaces_per_level() {
        let options = PrinterOptions {
            line_width: 80,
            indent_style: IndentStyle::Space,
            indent_width: 4,
            line_ending: LineEnding::Lf,
        };
        let doc = Document {
            elements: vec![indent(vec![hard_line_break(), text("b")])],
        };
        assert_eq!(print(&doc, &options), "\n    b");
    }
}
