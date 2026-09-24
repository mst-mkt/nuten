//! Prints a document as text.

use std::{collections::HashMap, iter, mem};

use crate::{Element, GroupId, LineMode, TextWidth, element::columns};

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

    /// What the printer trims from the end of a line before a break of its own.
    /// With `\n` line endings, a `\r` left there would be read again as part of the break.
    const fn trimmed_before_break(self) -> &'static [char] {
        match self {
            LineEnding::Lf => &[' ', '\t', '\r'],
            LineEnding::CrLf => &[' ', '\t'],
        }
    }

    /// `line` without what belongs to the `\n` that follows it in a text.
    /// The `\r` of a `\r\n` does.
    /// With `\n` line endings, so do all the `\r`s before it.
    /// They would be read again as part of the break.
    fn without_break(self, line: &str) -> &str {
        match self {
            LineEnding::Lf => line.trim_end_matches('\r'),
            LineEnding::CrLf => line.strip_suffix('\r').unwrap_or(line),
        }
    }
}

/// Spaces and tabs before a line break of the printer's go, even those of a text.
/// With `\n` line endings, so do `\r`s before any line break, a text's own included.
/// The lines inside a text stay as they are otherwise.
/// Their line breaks come out as the configured line ending.
/// The output never starts with a line break.
pub fn print(elements: &[Element<'_>], options: &PrinterOptions) -> String {
    let mut printer = Printer {
        options,
        stack: elements
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

/// The indent of a line goes out with its first text or space, so an empty line stays empty.
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
    /// Alignments since the last level.
    /// A deeper level turns them into tabs when indenting with tabs.
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

const HARD_LINE_BREAK: Element<'static> = Element::Line(LineMode::Hard {
    blank_lines: 0,
});

/// Whether the `Line`s of the group around print flat or broken.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    Flat,
    Break,
}

/// How `fits` measures an element: in the mode of its place, or inside a `LineSuffix`.
/// A `LineSuffix` among the measured elements takes no width.
/// It prints only at the next line break.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Measure {
    Flat,
    Break,
    InSuffix,
}

impl From<Mode> for Measure {
    fn from(mode: Mode) -> Measure {
        match mode {
            Mode::Flat => Measure::Flat,
            Mode::Break => Measure::Break,
        }
    }
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
                        let blank_lines = match *line_mode {
                            LineMode::Hard {
                                blank_lines,
                            } => blank_lines,
                            LineMode::Soft | LineMode::SoftOrSpace => 0,
                        };
                        self.push_line_breaks(indent, blank_lines + 1);
                    } else {
                        self.flush_line_suffixes(Some(frame));
                    }
                },
                Element::Space | Element::Line(LineMode::SoftOrSpace) => {
                    self.push_text(" ", TextWidth::Width(1));
                },
                Element::LineSuffixBoundary if !self.line_suffixes.is_empty() => {
                    // In break mode, so that the suffixes measure it as the end of the line.
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

    /// Whether `elements` fit on the rest of the line when printed flat.
    /// What follows them up to the next line break counts too.
    ///
    /// A hard line break, `ExpandParent`, or multi-line text never fits, even in a `LineSuffix`.
    /// So every group around one breaks.
    /// This replaces a separate pass that marks such groups.
    /// The width of a `LineSuffix` does not count.
    /// A `LineSuffixBoundary` after one ends the line, so what follows it is not measured.
    fn fits(&self, elements: &[Element<'_>]) -> bool {
        let Some(mut remaining) = self.options.line_width.checked_sub(self.column) else {
            return false;
        };
        let mut has_line_suffix = !self.line_suffixes.is_empty();
        let mut queue: Vec<(Measure, &Element<'_>)> = measured(elements, Measure::Flat).collect();
        let mut rest = self.stack.iter().rev();
        loop {
            let next = queue.pop().or_else(|| {
                rest.next()
                    .map(|frame| (Measure::from(frame.mode), frame.element))
            });
            let Some((measure, element)) = next else {
                return true;
            };
            let width = match (element, measure) {
                // The elements `Element::forces_break` names, spelled out.
                // A guard would take this arm out of the check that the match covers every pair.
                (
                    Element::Line(LineMode::Hard {
                        ..
                    })
                    | Element::ExpandParent
                    | Element::Text {
                        width: TextWidth::Multiline,
                        ..
                    },
                    Measure::Flat | Measure::InSuffix,
                ) => return false,
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
                    queue.extend(measured(elements, measure));
                    0
                },
                (Element::LineSuffix(elements), _) => {
                    has_line_suffix = true;
                    if measure != Measure::Break {
                        queue.extend(measured(elements, Measure::InSuffix));
                    }
                    0
                },
                (_, Measure::InSuffix) => 0,
                (Element::Line(_), Measure::Break) => return true,
                (
                    Element::Text {
                        width: TextWidth::Multiline,
                        text,
                    },
                    Measure::Break,
                ) => return columns(self.first_line(text)) <= remaining,
                (
                    Element::Text {
                        width: TextWidth::Width(width),
                        ..
                    },
                    _,
                ) => *width,
                (Element::Space | Element::Line(LineMode::SoftOrSpace), _) => 1,
                (Element::LineSuffixBoundary, _) if has_line_suffix => return true,
                (
                    Element::Line(LineMode::Soft)
                    | Element::ExpandParent
                    | Element::LineSuffixBoundary,
                    _,
                ) => 0,
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
        // The printer trims only before breaks of its own.
        // An indent here would stay at the end of the line.
        let empty_first_line = width == TextWidth::Multiline && self.first_line(text).is_empty();
        if let Some(indent) = self.pending_indent.take().filter(|_| !empty_first_line) {
            self.out.extend(iter::repeat_n('\t', indent.tabs as usize));
            self.out.extend(iter::repeat_n(' ', indent.spaces as usize));
        }
        match width {
            TextWidth::Width(width) => {
                self.out.push_str(text);
                self.column = self.column.saturating_add(width);
            },
            TextWidth::Multiline => {
                let line_ending = self.options.line_ending;
                let mut lines = text.split('\n');
                let last = lines.next_back().unwrap_or_default();
                for line in lines {
                    self.out.push_str(line_ending.without_break(line));
                    self.out.push_str(line_ending.as_str());
                }
                self.out.push_str(last);
                self.column = columns(last);
            },
        }
    }

    fn first_line<'t>(&self, text: &'t str) -> &'t str {
        text.split_once('\n').map_or(text, |(first, _)| {
            self.options.line_ending.without_break(first)
        })
    }

    /// Ends the line, dropping the spaces and tabs before the break.
    /// With `\n` line endings, it drops the `\r`s there too.
    /// Prints `breaks` line breaks, less those the output ends with already.
    /// A text's own line breaks count among those.
    /// Prints nothing at the start of the output, which counts as an empty line.
    fn push_line_breaks(&mut self, indent: Indentation, breaks: u32) {
        let trimmed = self
            .out
            .trim_end_matches(self.options.line_ending.trimmed_before_break())
            .len();
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

fn breaks_line(line_mode: LineMode, mode: Mode) -> bool {
    matches!(line_mode, LineMode::Hard { .. }) || mode == Mode::Break
}

/// In reverse, for a queue that pops from the end.
fn measured<'a>(
    elements: &'a [Element<'a>],
    measure: Measure,
) -> impl Iterator<Item = (Measure, &'a Element<'a>)> {
    elements.iter().rev().map(move |element| (measure, element))
}

#[cfg(test)]
mod tests {
    use super::{IndentStyle, LineEnding, PrinterOptions, print};
    use crate::{
        Element, GroupId, align, align_if_group_breaks, empty_line, empty_lines, expand_parent,
        group, group_with_id, hard_line_break, indent, line_suffix, line_suffix_boundary,
        soft_line_break, soft_line_break_or_space, space, text,
    };

    /// Wide enough that nothing breaks for want of room.
    const WIDE: PrinterOptions = PrinterOptions {
        line_width: 80,
        indent_style: IndentStyle::Space,
        indent_width: 2,
        line_ending: LineEnding::Lf,
    };

    const TABS: PrinterOptions = PrinterOptions {
        indent_style: IndentStyle::Tab,
        ..WIDE
    };

    const CRLF: PrinterOptions = PrinterOptions {
        line_ending: LineEnding::CrLf,
        ..WIDE
    };

    fn narrow(line_width: u32) -> PrinterOptions {
        PrinterOptions {
            line_width,
            ..WIDE
        }
    }

    fn comment() -> Element<'static> {
        line_suffix(vec![space(), text("# c")])
    }

    #[test]
    fn text_and_space_print_verbatim() {
        let doc = vec![text("echo"), space(), text("a")];
        assert_eq!(print(&doc, &WIDE), "echo a");
    }

    #[test]
    fn hard_line_break_indents_next_text() {
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
        assert_eq!(print(&doc, &WIDE), "a\n  b\n    c\nd");
        assert_eq!(print(&doc, &TABS), "a\n\tb\n\t\tc\nd");
    }

    #[test]
    fn indent_without_line_break_prints_nothing() {
        let doc = vec![indent(vec![text("a")])];
        assert_eq!(print(&doc, &WIDE), "a");
    }

    #[test]
    fn line_start_space_comes_after_indent() {
        let doc = vec![
            text("a"),
            indent(vec![hard_line_break(), space(), text("b")]),
        ];
        assert_eq!(print(&doc, &WIDE), "a\n   b");
    }

    #[test]
    fn trailing_spaces_and_tabs_are_trimmed() {
        let doc = vec![
            text("a "),
            space(),
            hard_line_break(),
            text("b\t"),
            empty_line(),
            text("c"),
        ];
        assert_eq!(print(&doc, &WIDE), "a\nb\n\nc");
    }

    #[test]
    fn consecutive_line_breaks_collapse() {
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
        assert_eq!(print(&doc, &WIDE), "a\nb\n\nc\n\nd\n\ne");
    }

    #[test]
    fn consecutive_line_breaks_collapse_with_crlf() {
        let doc = vec![
            text("a "),
            hard_line_break(),
            hard_line_break(),
            text("b"),
            empty_line(),
            empty_line(),
            text("c"),
        ];
        assert_eq!(print(&doc, &CRLF), "a\r\nb\r\n\r\nc");
    }

    #[test]
    fn consecutive_empty_lines_never_add_up() {
        let doc = vec![
            text("a"),
            empty_lines(2),
            empty_lines(2),
            text("b"),
            empty_line(),
            empty_lines(2),
            text("c"),
        ];
        assert_eq!(print(&doc, &WIDE), "a\n\n\nb\n\n\nc");
    }

    #[test]
    fn only_last_line_of_text_is_trimmed() {
        let doc = vec![text("a \nb "), hard_line_break(), text("c")];
        assert_eq!(print(&doc, &WIDE), "a \nb\nc");
    }

    #[test]
    fn only_lf_trims_cr_before_hard_line_break() {
        let doc = vec![text("a \r\r"), hard_line_break(), text("b\r \r")];
        assert_eq!(print(&doc, &WIDE), "a\nb\r \r");
        assert_eq!(print(&doc, &CRLF), "a \r\r\r\nb\r \r");
    }

    #[test]
    fn only_lf_trims_cr_before_line_break_in_text() {
        let doc = vec![text("a\r\r\nb \r\nc")];
        assert_eq!(print(&doc, &WIDE), "a\nb \nc");
        assert_eq!(print(&doc, &CRLF), "a\r\r\nb \r\nc");
        let doc = vec![text("a"), indent(vec![hard_line_break(), text("\r\r\nb")])];
        assert_eq!(print(&doc, &WIDE), "a\n\nb");
        assert_eq!(print(&doc, &CRLF), "a\r\n  \r\r\nb");
    }

    #[test]
    fn only_untrimmed_cr_takes_width() {
        let doc = vec![
            group(vec![text("a"), soft_line_break_or_space(), text("b")]),
            text("x\r\r\ny"),
        ];
        assert_eq!(print(&doc, &narrow(4)), "a bx\ny");
        let crlf = PrinterOptions {
            line_ending: LineEnding::CrLf,
            ..narrow(4)
        };
        assert_eq!(print(&doc, &crlf), "a\r\nbx\r\r\ny");
    }

    #[test]
    fn line_of_only_spaces_disappears() {
        let doc = vec![
            text("a"),
            hard_line_break(),
            space(),
            hard_line_break(),
            text("b"),
        ];
        assert_eq!(print(&doc, &WIDE), "a\nb");
    }

    #[test]
    fn output_never_starts_with_line_break() {
        let doc = vec![hard_line_break(), empty_line(), text("a")];
        assert_eq!(print(&doc, &WIDE), "a");
        let doc = vec![space(), hard_line_break(), text("a")];
        assert_eq!(print(&doc, &WIDE), "a");
        let doc = vec![indent(vec![hard_line_break(), text("a")])];
        assert_eq!(print(&doc, &WIDE), "  a");
    }

    #[test]
    fn empty_lines_carry_no_indent() {
        let doc = vec![text("a"), indent(vec![empty_line(), text("b")])];
        assert_eq!(print(&doc, &WIDE), "a\n\n  b");
    }

    #[test]
    fn final_line_break_leaves_no_indent_behind() {
        let doc = vec![indent(vec![text("a"), hard_line_break()])];
        assert_eq!(print(&doc, &WIDE), "a\n");
    }

    #[test]
    fn line_ending_sets_line_break_characters() {
        let doc = vec![text("a"), indent(vec![hard_line_break(), text("b")])];
        assert_eq!(print(&doc, &CRLF), "a\r\n  b");
    }

    #[test]
    fn indent_width_sets_spaces_per_level() {
        let doc = vec![text("a"), indent(vec![hard_line_break(), text("b")])];
        let four_column_indent = PrinterOptions {
            indent_width: 4,
            ..WIDE
        };
        assert_eq!(print(&doc, &four_column_indent), "a\n    b");
    }

    #[test]
    fn group_stays_flat_up_to_line_width() {
        let doc = vec![group(vec![
            text("aa"),
            soft_line_break_or_space(),
            text("bb"),
            soft_line_break(),
            text("cc"),
        ])];
        assert_eq!(print(&doc, &narrow(7)), "aa bbcc");
        assert_eq!(print(&doc, &narrow(6)), "aa\nbb\ncc");
    }

    #[test]
    fn group_starting_past_line_width_breaks() {
        let doc = vec![text("aaaa"), group(vec![soft_line_break(), text("b")])];
        assert_eq!(print(&doc, &narrow(3)), "aaaa\nb");
    }

    #[test]
    fn soft_lines_outside_group_break() {
        let doc = vec![text("a"), soft_line_break_or_space(), text("b")];
        assert_eq!(print(&doc, &WIDE), "a\nb");
    }

    #[test]
    fn group_inside_flat_group_stays_flat() {
        let doc = vec![group(vec![
            text("a"),
            soft_line_break_or_space(),
            group(vec![text("b"), soft_line_break(), text("c")]),
        ])];
        assert_eq!(print(&doc, &WIDE), "a bc");
    }

    #[test]
    fn forced_break_breaks_enclosing_groups() {
        let hard = vec![group(vec![
            text("a"),
            soft_line_break_or_space(),
            group(vec![text("b"), soft_line_break_or_space(), text("c")]),
            soft_line_break_or_space(),
            group(vec![text("d"), hard_line_break(), text("e")]),
        ])];
        assert_eq!(print(&hard, &WIDE), "a\nb c\nd\ne");
        let empty = vec![group(vec![text("a"), empty_line(), text("b")])];
        assert_eq!(print(&empty, &WIDE), "a\n\nb");
        let expand = vec![group(vec![
            text("a"),
            soft_line_break_or_space(),
            group(vec![text("b"), expand_parent(), text("c")]),
        ])];
        assert_eq!(print(&expand, &WIDE), "a\nbc");
        let multiline = vec![group(vec![
            text("a\nb"),
            soft_line_break_or_space(),
            text("c"),
        ])];
        assert_eq!(print(&multiline, &WIDE), "a\nb\nc");
        let in_indent = vec![group(vec![
            text("a"),
            indent(vec![
                soft_line_break_or_space(),
                text("b"),
                hard_line_break(),
                text("c"),
            ]),
        ])];
        assert_eq!(print(&in_indent, &WIDE), "a\n  b\n  c");
    }

    #[test]
    fn forced_break_in_line_suffix_breaks_enclosing_group() {
        let forced_breaks = [
            vec![expand_parent()],
            vec![hard_line_break()],
            vec![text("\n")],
            vec![indent(vec![group(vec![hard_line_break()])])],
            vec![line_suffix(vec![hard_line_break()])],
        ];
        for forced_break in forced_breaks {
            let doc = vec![group(vec![
                text("a"),
                line_suffix(forced_break.clone()),
                soft_line_break_or_space(),
                text("b"),
            ])];
            assert_eq!(print(&doc, &WIDE), "a\nb", "{forced_break:?}");
        }
    }

    #[test]
    fn boundary_in_line_suffix_never_ends_measurement() {
        let doc = vec![group(vec![
            text("a"),
            line_suffix(vec![line_suffix_boundary(), hard_line_break()]),
            soft_line_break_or_space(),
            text("b"),
        ])];
        assert_eq!(print(&doc, &WIDE), "a\nb");
    }

    #[test]
    fn group_ignores_forced_break_in_following_line_suffix() {
        let doc = vec![
            group(vec![text("a"), soft_line_break_or_space(), text("b")]),
            line_suffix(vec![hard_line_break(), text("# c")]),
        ];
        assert_eq!(print(&doc, &WIDE), "a b\n# c");
    }

    #[test]
    fn groups_inside_broken_group_fit_independently() {
        let doc = vec![group(vec![
            text("call"),
            soft_line_break_or_space(),
            group(vec![text("a"), soft_line_break_or_space(), text("b")]),
            soft_line_break_or_space(),
            group(vec![text("c"), soft_line_break_or_space(), text("d")]),
        ])];
        assert_eq!(print(&doc, &narrow(6)), "call\na b\nc d");
    }

    #[test]
    fn rest_of_line_counts_toward_group_fit() {
        let group_then = |rest: Vec<Element<'static>>| {
            let mut doc = vec![group(vec![
                text("a"),
                soft_line_break_or_space(),
                text("b"),
            ])];
            doc.extend(rest);
            doc
        };
        assert_eq!(print(&group_then(vec![text(";")]), &narrow(4)), "a b;");
        assert_eq!(print(&group_then(vec![text(";;")]), &narrow(4)), "a\nb;;");
        let then_line = vec![hard_line_break(), text("long line")];
        assert_eq!(print(&group_then(then_line), &narrow(4)), "a b\nlong line");
        let then_text = vec![text("x\r\ny")];
        assert_eq!(print(&group_then(then_text), &narrow(4)), "a bx\ny");
        let then_suffix = vec![line_suffix(vec![text("# a comment")])];
        assert_eq!(
            print(&group_then(then_suffix), &narrow(4)),
            "a b# a comment"
        );
    }

    #[test]
    fn indent_takes_columns() {
        let doc = vec![
            text("a"),
            indent(vec![
                hard_line_break(),
                group(vec![text("aa"), soft_line_break_or_space(), text("bb")]),
            ]),
        ];
        assert_eq!(print(&doc, &narrow(7)), "a\n  aa bb");
        assert_eq!(print(&doc, &narrow(6)), "a\n  aa\n  bb");
        let narrow_tabs = PrinterOptions {
            indent_style: IndentStyle::Tab,
            ..narrow(6)
        };
        assert_eq!(print(&doc, &narrow_tabs), "a\n\taa\n\tbb");
    }

    #[test]
    fn line_breaks_in_text_become_line_ending() {
        let doc = vec![text("a\r\nb\nc\rd"), hard_line_break(), text("e")];
        assert_eq!(print(&doc, &WIDE), "a\nb\nc\rd\ne");
        assert_eq!(print(&doc, &CRLF), "a\r\nb\r\nc\rd\r\ne");
        let doc = vec![text("a\n"), hard_line_break(), text("b")];
        assert_eq!(print(&doc, &WIDE), "a\nb");
        assert_eq!(print(&doc, &CRLF), "a\r\nb");
    }

    #[test]
    fn only_first_line_of_text_is_indented() {
        let doc = vec![text("a"), indent(vec![hard_line_break(), text("b\nc")])];
        assert_eq!(print(&doc, &WIDE), "a\n  b\nc");
    }

    #[test]
    fn empty_first_line_of_text_carries_no_indent() {
        for multiline in ["\nb", "\r\nb"] {
            let doc = vec![text("a"), indent(vec![hard_line_break(), text(multiline)])];
            assert_eq!(print(&doc, &WIDE), "a\n\nb", "{multiline:?}");
            assert_eq!(print(&doc, &TABS), "a\n\nb", "{multiline:?}");
        }
    }

    #[test]
    fn empty_text_leaves_indent_to_next_text() {
        let doc = vec![
            text("a"),
            indent(vec![hard_line_break(), text(""), text("b")]),
        ];
        assert_eq!(print(&doc, &WIDE), "a\n  b");
    }

    #[test]
    fn multiline_text_resets_column() {
        let doc = vec![
            text("a\nbb"),
            group(vec![text("c"), soft_line_break_or_space(), text("d")]),
        ];
        assert_eq!(print(&doc, &narrow(5)), "a\nbbc d");
    }

    #[test]
    fn line_suffix_keeps_written_indent_and_mode() {
        let doc = vec![
            text("a"),
            indent(vec![line_suffix(vec![hard_line_break(), text("# c")])]),
            hard_line_break(),
            text("b"),
        ];
        assert_eq!(print(&doc, &WIDE), "a\n  # c\nb");
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
        assert_eq!(print(&doc, &WIDE), "a b # c!\nd");
    }

    #[test]
    fn boundary_in_flat_group_keeps_line_suffix_group_flat() {
        let doc = vec![
            text("a"),
            line_suffix(vec![group(vec![
                text(" #"),
                soft_line_break_or_space(),
                text("c"),
            ])]),
            group(vec![line_suffix_boundary(), text("b")]),
        ];
        assert_eq!(print(&doc, &WIDE), "a # c\nb");
    }

    #[test]
    fn line_suffix_waits_for_next_line_break() {
        let doc = vec![
            text("a"),
            comment(),
            space(),
            text("b"),
            hard_line_break(),
            text("c"),
        ];
        assert_eq!(print(&doc, &WIDE), "a b # c\nc");
        let doc = vec![text("a"), comment(), empty_line(), text("b")];
        assert_eq!(print(&doc, &WIDE), "a # c\n\nb");
        let doc = vec![group(vec![
            text("a"),
            comment(),
            soft_line_break_or_space(),
            text("b"),
        ])];
        assert_eq!(print(&doc, &WIDE), "a b # c");
        assert_eq!(print(&doc, &narrow(2)), "a # c\nb");
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
        assert_eq!(print(&doc, &WIDE), "a x y\nb");
    }

    #[test]
    fn line_suffix_at_end_still_prints() {
        let doc = vec![text("a"), comment()];
        assert_eq!(print(&doc, &WIDE), "a # c");
    }

    #[test]
    fn boundary_breaks_line_only_after_line_suffix() {
        let doc = vec![text("a"), comment(), line_suffix_boundary(), text("b")];
        assert_eq!(print(&doc, &WIDE), "a # c\nb");
        let doc = vec![text("a"), line_suffix_boundary(), text("b")];
        assert_eq!(print(&doc, &WIDE), "ab");
    }

    #[test]
    fn line_suffix_takes_no_width() {
        let doc = vec![group(vec![
            text("aa"),
            line_suffix(vec![text(" # a long comment")]),
            soft_line_break_or_space(),
            text("bb"),
        ])];
        assert_eq!(print(&doc, &narrow(5)), "aa bb # a long comment");
    }

    #[test]
    fn boundary_after_line_suffix_ends_measurement() {
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
        assert_eq!(print(&doc, &narrow(7)), "aaa # c\nbbb ccc");
    }

    #[test]
    fn alignment_adds_its_columns_to_indent() {
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
        assert_eq!(print(&doc, &WIDE), "a\n  b\n    c\n   d");
    }

    #[test]
    fn alignment_before_indent_level_becomes_tab() {
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
        assert_eq!(print(&doc, &TABS), "a\n  b\n\t\tc\n\t\t d");
    }

    #[test]
    fn alignment_on_group_follows_whether_it_broke() {
        let doc = vec![
            group_with_id(
                GroupId(0),
                vec![text("aaa"), soft_line_break_or_space(), text("|")],
            ),
            align_if_group_breaks(
                GroupId(0),
                2,
                vec![space(), text("b"), hard_line_break(), text("c")],
            ),
        ];
        assert_eq!(print(&doc, &WIDE), "aaa | b\nc");
        assert_eq!(print(&doc, &narrow(4)), "aaa\n| b\n  c");
    }

    #[test]
    fn alignment_takes_columns() {
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
        assert_eq!(print(&doc, &narrow(8)), "a\n   aa bb");
        assert_eq!(print(&doc, &narrow(7)), "a\n   aa\n   bb");
    }

    #[test]
    fn alignment_takes_columns_after_tabs_too() {
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
        let four_column_tabs = |line_width| PrinterOptions {
            line_width,
            indent_width: 4,
            ..TABS
        };
        assert_eq!(print(&doc, &four_column_tabs(11)), "a\n\t  bb cc");
        assert_eq!(print(&doc, &four_column_tabs(10)), "a\n\t  bb\n\t  cc");
    }

    #[test]
    fn zero_columns_are_no_alignment() {
        let doc = vec![
            text("a"),
            align(0, vec![indent(vec![hard_line_break(), text("b")])]),
        ];
        assert_eq!(print(&doc, &TABS), "a\n\tb");
    }

    #[test]
    fn alignment_on_group_needs_it_already_broken() {
        let flat = vec![group(vec![
            group_with_id(GroupId(0), vec![text("a")]),
            align_if_group_breaks(GroupId(0), 2, vec![hard_line_break(), text("b")]),
        ])];
        assert_eq!(print(&flat, &WIDE), "a\nb");
        let in_flat_parent = vec![
            group(vec![group_with_id(
                GroupId(3),
                vec![text("x"), soft_line_break(), text("y")],
            )]),
            align_if_group_breaks(GroupId(3), 2, vec![hard_line_break(), text("b")]),
        ];
        assert_eq!(print(&in_flat_parent, &WIDE), "xy\nb");
        let unprinted = vec![
            align_if_group_breaks(GroupId(1), 2, vec![text("a"), hard_line_break(), text("b")]),
            group_with_id(GroupId(1), vec![text("c"), hard_line_break(), text("d")]),
        ];
        assert_eq!(print(&unprinted, &WIDE), "a\nbc\nd");
        let inside_own = vec![group_with_id(
            GroupId(2),
            vec![
                text("xx"),
                soft_line_break_or_space(),
                text("yy"),
                align_if_group_breaks(GroupId(2), 2, vec![hard_line_break(), text("z")]),
            ],
        )];
        assert_eq!(print(&inside_own, &WIDE), "xx\nyy\n  z");
    }

    #[test]
    fn alignment_spares_empty_lines_not_line_suffixes() {
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
        assert_eq!(print(&doc, &WIDE), "a\n\n  b\n  # c\nd");
    }
}
