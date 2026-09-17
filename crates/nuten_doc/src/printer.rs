//! Prints a document as text.

use std::{collections::HashMap, iter, mem};

use crate::{Document, Element, GroupId, LineMode, TextWidth, element::columns};

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
/// a text; the lines inside a text stay as they are, but their line breaks
/// come out as the configured line ending. The output never starts with a
/// line break.
pub fn print(document: &Document<'_>, options: &PrinterOptions) -> String {
    let mut printer = Printer {
        options,
        stack: document
            .elements
            .iter()
            .rev()
            .map(|element| Frame {
                indent: Indentation::default(),
                mode: Mode::Break,
                element,
            })
            .collect(),
        line_suffixes: Vec::new(),
        out: String::new(),
        group_modes: HashMap::new(),
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
    /// How the groups with an id printed.
    group_modes: HashMap<GroupId, Mode>,
    pending_indent: Option<Indentation>,
    /// Columns taken on the current line, a pending indent included.
    column: u32,
}

/// The indent of a line.
#[derive(Debug, Clone, Copy, Default)]
struct Indentation {
    tabs: u32,
    spaces: u32,
    /// Alignments since the last level, which a deeper level turns into
    /// tabs when indenting with tabs.
    aligns: u32,
}

#[derive(Debug, Clone, Copy)]
struct Frame<'a> {
    indent: Indentation,
    mode: Mode,
    element: &'a Element<'a>,
}

/// The contents of a `LineSuffix`, with the indent and mode of its place.
#[derive(Debug, Clone, Copy)]
struct Suffix<'a> {
    indent: Indentation,
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
                        let breaks = match *line_mode {
                            LineMode::Empty(blank_lines) => blank_lines + 1,
                            _ => 1,
                        };
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
                Element::Indent(elements) => {
                    let indent = self.deeper(indent);
                    self.push_elements(indent, mode, elements);
                },
                Element::Align {
                    columns,
                    group,
                    elements,
                } => {
                    let aligned = group
                        .is_none_or(|group| self.group_modes.get(&group) == Some(&Mode::Break));
                    let indent = if aligned {
                        self.aligned(indent, *columns)
                    } else {
                        indent
                    };
                    self.push_elements(indent, mode, elements);
                },
                Element::Group {
                    id,
                    elements,
                } => {
                    let mode = if mode == Mode::Flat || self.fits(elements) {
                        Mode::Flat
                    } else {
                        Mode::Break
                    };
                    if let Some(id) = id {
                        self.group_modes.insert(*id, mode);
                    }
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
                    Element::Line(LineMode::Hard | LineMode::Empty(_))
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
                (
                    Element::Group {
                        elements, ..
                    }
                    | Element::Indent(elements)
                    | Element::Align {
                        elements, ..
                    },
                    _,
                ) => {
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

    fn push_elements(&mut self, indent: Indentation, mode: Mode, elements: &'a [Element<'a>]) {
        self.stack
            .extend(elements.iter().rev().map(|element| Frame {
                indent,
                mode,
                element,
            }));
    }

    /// `indent` one level deeper.
    fn deeper(&self, indent: Indentation) -> Indentation {
        match self.options.indent_style {
            IndentStyle::Space => Indentation {
                spaces: indent
                    .spaces
                    .saturating_add(u32::from(self.options.indent_width)),
                ..indent
            },
            IndentStyle::Tab => Indentation {
                tabs: indent.tabs.saturating_add(indent.aligns).saturating_add(1),
                spaces: 0,
                aligns: 0,
            },
        }
    }

    /// `indent` with `columns` more; none at all for zero columns.
    fn aligned(&self, indent: Indentation, columns: u32) -> Indentation {
        if columns == 0 {
            return indent;
        }
        Indentation {
            tabs: indent.tabs,
            spaces: indent.spaces.saturating_add(columns),
            aligns: match self.options.indent_style {
                IndentStyle::Space => 0,
                IndentStyle::Tab => indent.aligns.saturating_add(1),
            },
        }
    }

    fn push_text(&mut self, text: &str, width: TextWidth) {
        if let Some(indent) = self.pending_indent.take() {
            self.out.extend(iter::repeat_n('\t', indent.tabs as usize));
            self.out.extend(iter::repeat_n(' ', indent.spaces as usize));
        }
        match width {
            TextWidth::Width(width) => {
                self.out.push_str(text);
                self.column = self.column.saturating_add(width);
            },
            TextWidth::Multiline => {
                let ending = self.options.line_ending.as_str();
                let mut lines = text.split('\n');
                let last = lines.next_back().unwrap_or_default();
                for line in lines {
                    self.out.push_str(without_cr(line));
                    self.out.push_str(ending);
                }
                self.out.push_str(last);
                self.column = columns(last);
            },
        }
    }

    /// Ends the line, dropping the spaces and tabs before the break, with
    /// `breaks` line breaks less those the output ends with already, a
    /// text's own included. Prints nothing at the start of the output,
    /// which counts as an empty line.
    fn push_line_breaks(&mut self, indent: Indentation, breaks: u32) {
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
        self.column = indent
            .tabs
            .saturating_mul(u32::from(self.options.indent_width))
            .saturating_add(indent.spaces);
    }
}

/// Whether a `Line` prints a line break in `mode`.
fn breaks_line(line_mode: LineMode, mode: Mode) -> bool {
    matches!(line_mode, LineMode::Hard | LineMode::Empty(_)) || mode == Mode::Break
}

/// The same elements `fits` rejects in flat mode, through a `LineSuffix`.
fn has_forced_break(elements: &[Element<'_>]) -> bool {
    elements.iter().any(|element| match element {
        Element::Line(LineMode::Hard | LineMode::Empty(_))
        | Element::ExpandParent
        | Element::Text {
            width: TextWidth::Multiline,
            ..
        } => true,
        Element::LineSuffix(elements)
        | Element::Group {
            elements, ..
        }
        | Element::Indent(elements)
        | Element::Align {
            elements, ..
        } => has_forced_break(elements),
        Element::Text {
            ..
        }
        | Element::Space
        | Element::Line(_)
        | Element::LineSuffixBoundary => false,
    })
}

fn first_line(text: &str) -> &str {
    without_cr(text.split_once('\n').map_or(text, |(first, _)| first))
}

/// `line` came before a `\n`; the `\r` of a `\r\n` is part of the break.
fn without_cr(line: &str) -> &str {
    line.strip_suffix('\r').unwrap_or(line)
}

#[cfg(test)]
mod tests {
    use super::{IndentStyle, LineEnding, PrinterOptions, print};
    use crate::{
        Document, Element, GroupId, align, align_if_group_breaks, empty_line, empty_lines,
        expand_parent, group, group_with_id, hard_line_break, indent, line_suffix,
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
    fn an_empty_line_prints_the_blank_lines_it_carries() {
        let doc = vec![
            text("a"),
            empty_lines(2),
            empty_lines(2),
            text("b"),
            empty_line(),
            empty_lines(2),
            text("c"),
        ];
        assert_eq!(printed(doc, &options(80)), "a\n\n\nb\n\n\nc");
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
        assert_eq!(printed(group_then(then_text), &options(4)), "a bx\ny");
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
    fn line_breaks_inside_a_text_come_out_as_the_line_ending() {
        let doc = || vec![text("a\r\nb\nc\rd"), hard_line_break(), text("e")];
        assert_eq!(printed(doc(), &options(80)), "a\nb\nc\rd\ne");
        let crlf = PrinterOptions {
            line_ending: LineEnding::CrLf,
            ..options(80)
        };
        assert_eq!(printed(doc(), &crlf), "a\r\nb\r\nc\rd\r\ne");
        let doc = || vec![text("a\n"), hard_line_break(), text("b")];
        assert_eq!(printed(doc(), &options(80)), "a\nb");
        assert_eq!(printed(doc(), &crlf), "a\r\nb");
    }

    #[test]
    fn only_the_first_line_of_a_text_is_indented() {
        let doc = vec![text("a"), indent(vec![hard_line_break(), text("b\nc")])];
        assert_eq!(printed(doc, &options(80)), "a\n  b\nc");
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

    #[test]
    fn an_alignment_adds_its_columns_to_the_indent() {
        let doc = vec![
            text("a"),
            align(
                2,
                vec![
                    hard_line_break(),
                    text("b"),
                    indent(vec![hard_line_break(), text("c")]),
                ],
            ),
            indent(vec![align(1, vec![hard_line_break(), text("d")])]),
        ];
        assert_eq!(printed(doc, &options(80)), "a\n  b\n    c\n   d");
    }

    #[test]
    fn with_tabs_an_alignment_before_a_level_takes_a_tab() {
        let tabs = PrinterOptions {
            indent_style: IndentStyle::Tab,
            ..options(80)
        };
        let doc = vec![
            text("a"),
            align(
                2,
                vec![
                    hard_line_break(),
                    text("b"),
                    indent(vec![
                        hard_line_break(),
                        text("c"),
                        align(1, vec![hard_line_break(), text("d")]),
                    ]),
                ],
            ),
        ];
        assert_eq!(printed(doc, &tabs), "a\n  b\n\t\tc\n\t\t d");
    }

    #[test]
    fn an_alignment_on_a_group_follows_how_the_group_printed() {
        let doc = |width| {
            printed(
                vec![
                    group_with_id(
                        GroupId(0),
                        vec![text("aaa"), soft_line_break_or_space(), text("|")],
                    ),
                    align_if_group_breaks(
                        GroupId(0),
                        2,
                        vec![space(), text("b"), hard_line_break(), text("c")],
                    ),
                ],
                &options(width),
            )
        };
        assert_eq!(doc(80), "aaa | b\nc");
        assert_eq!(doc(4), "aaa\n| b\n  c");
    }

    #[test]
    fn an_alignment_takes_columns() {
        let doc = vec![
            text("a"),
            align(
                3,
                vec![
                    hard_line_break(),
                    group(vec![text("aa"), soft_line_break_or_space(), text("bb")]),
                ],
            ),
        ];
        assert_eq!(printed(doc.clone(), &options(8)), "a\n   aa bb");
        assert_eq!(printed(doc, &options(7)), "a\n   aa\n   bb");
    }

    #[test]
    fn an_alignment_takes_columns_after_tabs_too() {
        let doc = |line_width| {
            let tabs = PrinterOptions {
                line_width,
                indent_style: IndentStyle::Tab,
                indent_width: 4,
                line_ending: LineEnding::Lf,
            };
            let doc = vec![
                text("a"),
                indent(vec![align(
                    2,
                    vec![
                        hard_line_break(),
                        group(vec![text("bb"), soft_line_break_or_space(), text("cc")]),
                    ],
                )]),
            ];
            printed(doc, &tabs)
        };
        assert_eq!(doc(11), "a\n\t  bb cc");
        assert_eq!(doc(10), "a\n\t  bb\n\t  cc");
    }

    #[test]
    fn zero_columns_are_no_alignment() {
        let tabs = PrinterOptions {
            indent_style: IndentStyle::Tab,
            ..options(80)
        };
        let doc = vec![
            text("a"),
            align(0, vec![indent(vec![hard_line_break(), text("b")])]),
        ];
        assert_eq!(printed(doc, &tabs), "a\n\tb");
    }

    #[test]
    fn an_alignment_on_a_flat_or_unprinted_group_does_nothing() {
        let flat = vec![group(vec![
            group_with_id(GroupId(0), vec![text("a")]),
            align_if_group_breaks(GroupId(0), 2, vec![hard_line_break(), text("b")]),
        ])];
        assert_eq!(printed(flat, &options(80)), "a\nb");
        let in_flat_parent = vec![
            group(vec![group_with_id(
                GroupId(3),
                vec![text("x"), soft_line_break(), text("y")],
            )]),
            align_if_group_breaks(GroupId(3), 2, vec![hard_line_break(), text("b")]),
        ];
        assert_eq!(printed(in_flat_parent, &options(80)), "xy\nb");
        let unprinted = vec![
            align_if_group_breaks(GroupId(1), 2, vec![text("a"), hard_line_break(), text("b")]),
            group_with_id(GroupId(1), vec![text("c"), hard_line_break(), text("d")]),
        ];
        assert_eq!(printed(unprinted, &options(80)), "a\nbc\nd");
        let inside_own = vec![group_with_id(
            GroupId(2),
            vec![
                text("xx"),
                soft_line_break_or_space(),
                text("yy"),
                align_if_group_breaks(GroupId(2), 2, vec![hard_line_break(), text("z")]),
            ],
        )];
        assert_eq!(printed(inside_own, &options(80)), "xx\nyy\n  z");
    }

    #[test]
    fn an_alignment_leaves_empty_lines_empty_and_suffixes_in_place() {
        let doc = vec![
            text("a"),
            align(
                2,
                vec![
                    empty_line(),
                    text("b"),
                    line_suffix(vec![hard_line_break(), text("# c")]),
                ],
            ),
            hard_line_break(),
            text("d"),
        ];
        assert_eq!(printed(doc, &options(80)), "a\n\n  b\n  # c\nd");
    }
}
