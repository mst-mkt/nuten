//! Prints a document as text.

use std::{iter, mem};

use crate::{Document, Element, LineMode, TextWidth, element::columns};

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

/// Spaces and tabs before a line break of the printer's go, even those of
/// a text; the lines inside a text stay as they are. The output never
/// starts with a line break.
pub fn print(document: &Document<'_>, options: &PrinterOptions) -> String {
    let mut printer = Printer {
        options,
        stack: document
            .elements
            .iter()
            .rev()
            .map(|element| Frame {
                indent: 0,
                mode: Mode::Break,
                element,
            })
            .collect(),
        line_suffixes: Vec::new(),
        out: String::new(),
        pending_indent: None,
        column: 0,
    };
    printer.print_all();
    printer.out
}

/// The indent of a line goes out with its first text or space, so an empty
/// line stays empty.
struct Printer<'a> {
    options: &'a PrinterOptions,
    /// What is left to print, the next element on top.
    stack: Vec<Frame<'a>>,
    /// Held back for the next line break, in order.
    line_suffixes: Vec<Suffix<'a>>,
    out: String,
    pending_indent: Option<u32>,
    /// Columns taken on the current line, a pending indent included.
    column: u32,
}

#[derive(Debug, Clone, Copy)]
struct Frame<'a> {
    indent: u32,
    mode: Mode,
    element: &'a Element<'a>,
}

/// The contents of a `LineSuffix`, with the indent and mode of its place.
#[derive(Debug, Clone, Copy)]
struct Suffix<'a> {
    indent: u32,
    mode: Mode,
    elements: &'a [Element<'a>],
}

const HARD_LINE_BREAK: Element<'static> = Element::Line(LineMode::Hard);

/// Whether the `Line`s of the group around print flat or broken.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    Flat,
    Break,
}

impl<'a> Printer<'a> {
    fn print_all(&mut self) {
        loop {
            let Some(frame) = self.stack.pop() else {
                if self.line_suffixes.is_empty() {
                    break;
                }
                self.flush_line_suffixes(None);
                continue;
            };
            let Frame {
                indent,
                mode,
                element,
            } = frame;
            match element {
                Element::Text {
                    text,
                    width,
                } => self.push_text(text, *width),
                Element::Line(line_mode) if breaks_line(*line_mode, mode) => {
                    if self.line_suffixes.is_empty() {
                        let breaks = if *line_mode == LineMode::Empty { 2 } else { 1 };
                        self.push_line_breaks(indent, breaks);
                    } else {
                        self.flush_line_suffixes(Some(frame));
                    }
                },
                Element::Space | Element::Line(LineMode::SoftOrSpace) => {
                    self.push_text(" ", TextWidth::Width(1));
                },
                Element::LineSuffixBoundary if !self.line_suffixes.is_empty() => {
                    // In break mode, so that the suffixes measure it as the
                    // end of the line.
                    self.flush_line_suffixes(Some(Frame {
                        indent,
                        mode: Mode::Break,
                        element: &HARD_LINE_BREAK,
                    }));
                },
                // Only `Soft` in flat mode reaches this arm among the lines.
                Element::Line(_) | Element::ExpandParent | Element::LineSuffixBoundary => {},
                Element::LineSuffix(elements) => self.line_suffixes.push(Suffix {
                    indent,
                    mode,
                    elements,
                }),
                Element::Indent(elements) => self.push_elements(indent + 1, mode, elements),
                Element::Group(elements) => {
                    let mode = if mode == Mode::Flat || self.fits(elements) {
                        Mode::Flat
                    } else {
                        Mode::Break
                    };
                    self.push_elements(indent, mode, elements);
                },
            }
        }
    }

    /// Whether `elements` fit on the rest of the line when printed flat,
    /// along with what follows them up to the next line break.
    ///
    /// A hard line break, `ExpandParent`, or multi-line text in `elements`
    /// never fits, even inside a `LineSuffix`, so every group around one
    /// breaks. This replaces a separate pass that marks such groups.
    /// The width of a `LineSuffix` does not count, and a
    /// `LineSuffixBoundary` after one ends the line: what follows it is not
    /// measured.
    fn fits(&self, elements: &[Element<'_>]) -> bool {
        let Some(mut remaining) = self.options.line_width.checked_sub(self.column) else {
            return false;
        };
        let mut has_line_suffix = !self.line_suffixes.is_empty();
        let mut queue: Vec<(Mode, &Element<'_>)> =
            elements.iter().rev().map(|e| (Mode::Flat, e)).collect();
        // Every frame is in break mode here: a group inside a flat group is
        // never measured.
        let mut rest = self.stack.iter().rev();
        loop {
            let next = queue
                .pop()
                .or_else(|| rest.next().map(|frame| (frame.mode, frame.element)));
            let Some((mode, element)) = next else {
                return true;
            };
            let width = match (element, mode) {
                (
                    Element::Line(LineMode::Hard | LineMode::Empty)
                    | Element::ExpandParent
                    | Element::Text {
                        width: TextWidth::Multiline,
                        ..
                    },
                    Mode::Flat,
                ) => return false,
                (Element::LineSuffix(elements), Mode::Flat) if has_forced_break(elements) => {
                    return false;
                },
                (Element::Line(_), Mode::Break) => return true,
                (
                    Element::Text {
                        width: TextWidth::Multiline,
                        text,
                    },
                    Mode::Break,
                ) => return columns(first_line(text)) <= remaining,
                (
                    Element::Text {
                        width: TextWidth::Width(width),
                        ..
                    },
                    _,
                ) => *width,
                (Element::Space | Element::Line(LineMode::SoftOrSpace), _) => 1,
                (Element::LineSuffix(_), _) => {
                    has_line_suffix = true;
                    0
                },
                (Element::LineSuffixBoundary, _) if has_line_suffix => return true,
                (
                    Element::Line(LineMode::Soft)
                    | Element::ExpandParent
                    | Element::LineSuffixBoundary,
                    _,
                ) => 0,
                (Element::Group(elements) | Element::Indent(elements), _) => {
                    queue.extend(elements.iter().rev().map(|e| (mode, e)));
                    0
                },
            };
            let Some(left) = remaining.checked_sub(width) else {
                return false;
            };
            remaining = left;
        }
    }

    /// Puts the held-back suffixes on the stack, in order, before `then`.
    /// A `LineSuffix` among them is held back again.
    fn flush_line_suffixes(&mut self, then: Option<Frame<'a>>) {
        if let Some(then) = then {
            self.stack.push(then);
        }
        for suffix in mem::take(&mut self.line_suffixes).into_iter().rev() {
            self.push_elements(suffix.indent, suffix.mode, suffix.elements);
        }
    }

    fn push_elements(&mut self, indent: u32, mode: Mode, elements: &'a [Element<'a>]) {
        self.stack
            .extend(elements.iter().rev().map(|element| Frame {
                indent,
                mode,
                element,
            }));
    }

    fn push_text(&mut self, text: &str, width: TextWidth) {
        if let Some(indent) = self.pending_indent.take() {
            let (unit, count) = match self.options.indent_style {
                IndentStyle::Tab => ('\t', indent),
                IndentStyle::Space => (' ', indent * u32::from(self.options.indent_width)),
            };
            self.out.extend(iter::repeat_n(unit, count as usize));
        }
        self.out.push_str(text);
        self.column = match width {
            TextWidth::Width(width) => self.column.saturating_add(width),
            TextWidth::Multiline => columns(last_line(text)),
        };
    }

    /// Ends the line, dropping the spaces and tabs before the break, with
    /// `breaks` line breaks less those the output ends with already, a
    /// text's own included. Prints nothing at the start of the output,
    /// which counts as an empty line.
    fn push_line_breaks(&mut self, indent: u32, breaks: usize) {
        let trimmed = self.out.trim_end_matches([' ', '\t']).len();
        self.out.truncate(trimmed);
        let ending = self.options.line_ending.as_str();
        let mut existing = if self.out.is_empty() { breaks } else { 0 };
        let mut rest = self.out.as_str();
        while existing < breaks {
            let Some(before) = rest.strip_suffix(ending) else {
                break;
            };
            rest = before;
            existing += 1;
        }
        for _ in existing..breaks {
            self.out.push_str(ending);
        }
        self.pending_indent = Some(indent);
        self.column = indent * u32::from(self.options.indent_width);
    }
}

/// Whether a `Line` prints a line break in `mode`.
fn breaks_line(line_mode: LineMode, mode: Mode) -> bool {
    matches!(line_mode, LineMode::Hard | LineMode::Empty) || mode == Mode::Break
}

/// The same elements `fits` rejects in flat mode, through a `LineSuffix`.
fn has_forced_break(elements: &[Element<'_>]) -> bool {
    elements.iter().any(|element| match element {
        Element::Line(LineMode::Hard | LineMode::Empty)
        | Element::ExpandParent
        | Element::Text {
            width: TextWidth::Multiline,
            ..
        } => true,
        Element::LineSuffix(elements) | Element::Group(elements) | Element::Indent(elements) => {
            has_forced_break(elements)
        },
        Element::Text {
            ..
        }
        | Element::Space
        | Element::Line(_)
        | Element::LineSuffixBoundary => false,
    })
}

fn first_line(text: &str) -> &str {
    let first = text.split_once('\n').map_or(text, |(first, _)| first);
    first.strip_suffix('\r').unwrap_or(first)
}

fn last_line(text: &str) -> &str {
    text.rsplit_once('\n').map_or(text, |(_, last)| last)
}

#[cfg(test)]
mod tests {
    use super::{IndentStyle, LineEnding, PrinterOptions, print};
    use crate::{
        Document, Element, empty_line, expand_parent, group, hard_line_break, indent, line_suffix,
        line_suffix_boundary, soft_line_break, soft_line_break_or_space, space, text,
    };

    fn options(line_width: u32) -> PrinterOptions {
        PrinterOptions {
            line_width,
            indent_style: IndentStyle::Space,
            indent_width: 2,
            line_ending: LineEnding::Lf,
        }
    }

    fn printed(elements: Vec<Element<'_>>, options: &PrinterOptions) -> String {
        print(
            &Document {
                elements,
            },
            options,
        )
    }

    fn comment() -> Element<'static> {
        line_suffix(vec![space(), text("# c")])
    }

    #[test]
    fn text_and_space_go_out_as_they_are() {
        let doc = vec![text("echo"), space(), text("a")];
        assert_eq!(printed(doc, &options(80)), "echo a");
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
        assert_eq!(printed(doc.clone(), &options(80)), "a\n  b\n    c\nd");
        let tabs = PrinterOptions {
            indent_style: IndentStyle::Tab,
            ..options(80)
        };
        assert_eq!(printed(doc, &tabs), "a\n\tb\n\t\tc\nd");
    }

    #[test]
    fn indent_without_a_line_break_prints_nothing() {
        let doc = vec![indent(vec![text("a")])];
        assert_eq!(printed(doc, &options(80)), "a");
    }

    #[test]
    fn a_space_at_the_start_of_a_line_comes_after_the_indent() {
        let doc = vec![
            text("a"),
            indent(vec![hard_line_break(), space(), text("b")]),
        ];
        assert_eq!(printed(doc, &options(80)), "a\n   b");
    }

    #[test]
    fn spaces_and_tabs_at_the_end_of_a_line_go() {
        let doc = vec![
            text("a "),
            space(),
            hard_line_break(),
            text("b\t"),
            empty_line(),
            text("c"),
        ];
        assert_eq!(printed(doc, &options(80)), "a\nb\n\nc");
    }

    #[test]
    fn line_breaks_in_a_row_collapse() {
        let doc = vec![
            text("a"),
            hard_line_break(),
            hard_line_break(),
            text("b"),
            hard_line_break(),
            empty_line(),
            text("c"),
            empty_line(),
            hard_line_break(),
            text("d"),
            empty_line(),
            empty_line(),
            text("e"),
        ];
        assert_eq!(printed(doc, &options(80)), "a\nb\n\nc\n\nd\n\ne");
    }

    #[test]
    fn line_breaks_collapse_with_crlf_too() {
        let doc = vec![
            text("a "),
            hard_line_break(),
            hard_line_break(),
            text("b"),
            empty_line(),
            empty_line(),
            text("c"),
        ];
        let crlf = PrinterOptions {
            line_ending: LineEnding::CrLf,
            ..options(80)
        };
        assert_eq!(printed(doc, &crlf), "a\r\nb\r\n\r\nc");
    }

    #[test]
    fn only_the_last_line_of_a_text_is_trimmed() {
        let doc = vec![text("a \nb "), hard_line_break(), text("c")];
        assert_eq!(printed(doc, &options(80)), "a \nb\nc");
    }

    #[test]
    fn a_line_of_spaces_alone_goes() {
        let doc = vec![
            text("a"),
            hard_line_break(),
            space(),
            hard_line_break(),
            text("b"),
        ];
        assert_eq!(printed(doc, &options(80)), "a\nb");
    }

    #[test]
    fn the_output_never_starts_with_a_line_break() {
        let doc = vec![hard_line_break(), empty_line(), text("a")];
        assert_eq!(printed(doc, &options(80)), "a");
        let doc = vec![space(), hard_line_break(), text("a")];
        assert_eq!(printed(doc, &options(80)), "a");
        let doc = vec![indent(vec![hard_line_break(), text("a")])];
        assert_eq!(printed(doc, &options(80)), "  a");
    }

    #[test]
    fn empty_lines_carry_no_indent() {
        let doc = vec![text("a"), indent(vec![empty_line(), text("b")])];
        assert_eq!(printed(doc, &options(80)), "a\n\n  b");
    }

    #[test]
    fn a_final_line_break_leaves_no_indent_behind() {
        let doc = vec![indent(vec![text("a"), hard_line_break()])];
        assert_eq!(printed(doc, &options(80)), "a\n");
    }

    #[test]
    fn line_ending_is_configurable() {
        let doc = vec![text("a"), indent(vec![hard_line_break(), text("b")])];
        let crlf = PrinterOptions {
            line_ending: LineEnding::CrLf,
            ..options(80)
        };
        assert_eq!(printed(doc, &crlf), "a\r\n  b");
    }

    #[test]
    fn indent_width_sets_the_spaces_per_level() {
        let doc = vec![text("a"), indent(vec![hard_line_break(), text("b")])];
        let wide = PrinterOptions {
            indent_width: 4,
            ..options(80)
        };
        assert_eq!(printed(doc, &wide), "a\n    b");
    }

    #[test]
    fn a_group_stays_flat_up_to_the_line_width() {
        let doc = || {
            vec![group(vec![
                text("aa"),
                soft_line_break_or_space(),
                text("bb"),
                soft_line_break(),
                text("cc"),
            ])]
        };
        assert_eq!(printed(doc(), &options(7)), "aa bbcc");
        assert_eq!(printed(doc(), &options(6)), "aa\nbb\ncc");
    }

    #[test]
    fn a_group_past_the_line_width_breaks_at_once() {
        let doc = vec![text("aaaa"), group(vec![soft_line_break(), text("b")])];
        assert_eq!(printed(doc, &options(3)), "aaaa\nb");
    }

    #[test]
    fn soft_lines_outside_a_group_break() {
        let doc = vec![text("a"), soft_line_break_or_space(), text("b")];
        assert_eq!(printed(doc, &options(80)), "a\nb");
    }

    #[test]
    fn a_group_inside_a_flat_group_stays_flat() {
        let doc = vec![group(vec![
            text("a"),
            soft_line_break_or_space(),
            group(vec![text("b"), soft_line_break(), text("c")]),
        ])];
        assert_eq!(printed(doc, &options(80)), "a bc");
    }

    #[test]
    fn a_forced_break_breaks_the_groups_around() {
        let hard = vec![group(vec![
            text("a"),
            soft_line_break_or_space(),
            group(vec![text("b"), soft_line_break_or_space(), text("c")]),
            soft_line_break_or_space(),
            group(vec![text("d"), hard_line_break(), text("e")]),
        ])];
        assert_eq!(printed(hard, &options(80)), "a\nb c\nd\ne");
        let empty = vec![group(vec![text("a"), empty_line(), text("b")])];
        assert_eq!(printed(empty, &options(80)), "a\n\nb");
        let expand = vec![group(vec![
            text("a"),
            soft_line_break_or_space(),
            group(vec![text("b"), expand_parent(), text("c")]),
        ])];
        assert_eq!(printed(expand, &options(80)), "a\nbc");
        let multiline = vec![group(vec![
            text("a\nb"),
            soft_line_break_or_space(),
            text("c"),
        ])];
        assert_eq!(printed(multiline, &options(80)), "a\nb\nc");
        let in_indent = vec![group(vec![
            text("a"),
            indent(vec![
                soft_line_break_or_space(),
                text("b"),
                hard_line_break(),
                text("c"),
            ]),
        ])];
        assert_eq!(printed(in_indent, &options(80)), "a\n  b\n  c");
    }

    #[test]
    fn a_forced_break_inside_a_line_suffix_breaks_the_group_around() {
        let doc = vec![group(vec![
            text("a"),
            line_suffix(vec![expand_parent()]),
            soft_line_break_or_space(),
            text("b"),
        ])];
        assert_eq!(printed(doc, &options(80)), "a\nb");
    }

    #[test]
    fn groups_inside_a_broken_group_fit_on_their_own() {
        let doc = vec![group(vec![
            text("call"),
            soft_line_break_or_space(),
            group(vec![text("a"), soft_line_break_or_space(), text("b")]),
            soft_line_break_or_space(),
            group(vec![text("c"), soft_line_break_or_space(), text("d")]),
        ])];
        assert_eq!(printed(doc, &options(6)), "call\na b\nc d");
    }

    #[test]
    fn what_follows_a_group_on_the_line_counts() {
        let group_then = |rest: Vec<Element<'static>>| {
            let mut doc = vec![group(vec![
                text("a"),
                soft_line_break_or_space(),
                text("b"),
            ])];
            doc.extend(rest);
            doc
        };
        assert_eq!(printed(group_then(vec![text(";")]), &options(4)), "a b;");
        assert_eq!(printed(group_then(vec![text(";;")]), &options(4)), "a\nb;;");
        let then_line = vec![hard_line_break(), text("long line")];
        assert_eq!(
            printed(group_then(then_line), &options(4)),
            "a b\nlong line"
        );
        let then_text = vec![text("x\r\ny")];
        assert_eq!(printed(group_then(then_text), &options(4)), "a bx\r\ny");
        let then_suffix = vec![line_suffix(vec![text("# a comment")])];
        assert_eq!(
            printed(group_then(then_suffix), &options(4)),
            "a b# a comment"
        );
    }

    #[test]
    fn the_indent_takes_columns() {
        let doc = vec![
            text("a"),
            indent(vec![
                hard_line_break(),
                group(vec![text("aa"), soft_line_break_or_space(), text("bb")]),
            ]),
        ];
        assert_eq!(printed(doc.clone(), &options(7)), "a\n  aa bb");
        assert_eq!(printed(doc.clone(), &options(6)), "a\n  aa\n  bb");
        let tabs = PrinterOptions {
            indent_style: IndentStyle::Tab,
            ..options(6)
        };
        assert_eq!(printed(doc, &tabs), "a\n\taa\n\tbb");
    }

    #[test]
    fn a_multiline_text_resets_the_column() {
        let doc = vec![
            text("a\nbb"),
            group(vec![text("c"), soft_line_break_or_space(), text("d")]),
        ];
        assert_eq!(printed(doc, &options(5)), "a\nbbc d");
    }

    #[test]
    fn a_line_suffix_keeps_the_indent_and_mode_of_its_place() {
        let doc = vec![
            text("a"),
            indent(vec![line_suffix(vec![hard_line_break(), text("# c")])]),
            hard_line_break(),
            text("b"),
        ];
        assert_eq!(printed(doc, &options(80)), "a\n  # c\nb");
        let doc = vec![
            group(vec![
                text("a"),
                line_suffix(vec![space(), text("# c"), soft_line_break(), text("!")]),
                space(),
                text("b"),
            ]),
            hard_line_break(),
            text("d"),
        ];
        assert_eq!(printed(doc, &options(80)), "a b # c!\nd");
    }

    #[test]
    fn a_boundary_in_a_flat_group_keeps_a_suffix_group_flat() {
        let doc = vec![
            text("a"),
            line_suffix(vec![group(vec![
                text(" #"),
                soft_line_break_or_space(),
                text("c"),
            ])]),
            group(vec![line_suffix_boundary(), text("b")]),
        ];
        assert_eq!(printed(doc, &options(80)), "a # c\nb");
    }

    #[test]
    fn a_line_suffix_waits_for_the_next_line_break() {
        let doc = vec![
            text("a"),
            comment(),
            space(),
            text("b"),
            hard_line_break(),
            text("c"),
        ];
        assert_eq!(printed(doc, &options(80)), "a b # c\nc");
        let doc = vec![text("a"), comment(), empty_line(), text("b")];
        assert_eq!(printed(doc, &options(80)), "a # c\n\nb");
        let doc = vec![group(vec![
            text("a"),
            comment(),
            soft_line_break_or_space(),
            text("b"),
        ])];
        assert_eq!(printed(doc.clone(), &options(80)), "a b # c");
        assert_eq!(printed(doc, &options(2)), "a # c\nb");
    }

    #[test]
    fn line_suffixes_keep_their_order() {
        let doc = vec![
            text("a"),
            line_suffix(vec![text(" x")]),
            line_suffix(vec![text(" y")]),
            hard_line_break(),
            text("b"),
        ];
        assert_eq!(printed(doc, &options(80)), "a x y\nb");
    }

    #[test]
    fn a_line_suffix_at_the_end_still_prints() {
        let doc = vec![text("a"), comment()];
        assert_eq!(printed(doc, &options(80)), "a # c");
    }

    #[test]
    fn a_boundary_breaks_the_line_only_after_a_suffix() {
        let doc = vec![text("a"), comment(), line_suffix_boundary(), text("b")];
        assert_eq!(printed(doc, &options(80)), "a # c\nb");
        let doc = vec![text("a"), line_suffix_boundary(), text("b")];
        assert_eq!(printed(doc, &options(80)), "ab");
    }

    #[test]
    fn a_line_suffix_takes_no_width() {
        let doc = vec![group(vec![
            text("aa"),
            line_suffix(vec![text(" # a long comment")]),
            soft_line_break_or_space(),
            text("bb"),
        ])];
        assert_eq!(printed(doc, &options(5)), "aa bb # a long comment");
    }

    #[test]
    fn a_boundary_after_a_suffix_ends_the_line_when_measuring() {
        let doc = vec![
            text("aaa"),
            comment(),
            group(vec![
                line_suffix_boundary(),
                text("bbb"),
                soft_line_break_or_space(),
                text("ccc"),
            ]),
        ];
        assert_eq!(printed(doc, &options(7)), "aaa # c\nbbb ccc");
    }
}
