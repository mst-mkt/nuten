use std::{collections::HashMap, iter, mem, slice};

use crate::{Columns, Document, Element, GroupId, LineMode, TextWidth, element::columns};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrinterOptions {
    pub line_width: u32,
    pub indent_style: IndentStyle,
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

    /// With `\n` line endings, a `\r` left there would be read again as part of the break.
    const fn trimmed_before_break(self) -> &'static [char] {
        match self {
            LineEnding::Lf => &[' ', '\t', '\r'],
            LineEnding::CrLf => &[' ', '\t'],
        }
    }

    /// Every `\r` goes with `\n` line endings, for the same reason as in `trimmed_before_break`.
    fn without_break(self, line: &str) -> &str {
        match self {
            LineEnding::Lf => line.trim_end_matches('\r'),
            LineEnding::CrLf => line.strip_suffix('\r').unwrap_or(line),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TextBreaks {
    AsLineEnding,
    AsWritten,
}

pub fn print(document: &Document<'_>, options: &PrinterOptions) -> String {
    let mut printer = Printer {
        options,
        document,
        stack: document
            .root()
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

struct Printer<'a> {
    options: &'a PrinterOptions,
    document: &'a Document<'a>,
    stack: Vec<Frame<'a>>,
    line_suffixes: Vec<Suffix<'a>>,
    out: String,
    group_modes: HashMap<GroupId, Mode>,
    /// Goes out with the first text or space of the line, so an empty line stays empty.
    pending_indent: Option<Indentation>,
    column: u32,
}

#[derive(Debug, Clone, Copy, Default)]
struct Indentation {
    tabs: u32,
    spaces: u32,
    /// Counted so that a deeper level can turn them into tabs when indenting with tabs.
    aligns: u32,
}

#[derive(Debug, Clone, Copy)]
struct Frame<'a> {
    indent: Indentation,
    mode: Mode,
    element: &'a Element<'a>,
}

#[derive(Debug, Clone, Copy)]
struct Suffix<'a> {
    indent: Indentation,
    mode: Mode,
    elements: &'a [Element<'a>],
}

const HARD_LINE_BREAK: Element<'static> = Element::Line(LineMode::Hard {
    blank_lines: 0,
});

const SPACE_WIDTH: TextWidth = TextWidth::Width(Columns::new(1));

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    Flat,
    Break,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Measure {
    Flat,
    Break,
    /// Takes no width, since a suffix prints only at the next line break.
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
                } => self.push_text(text, *width, TextBreaks::AsLineEnding),
                Element::ExactText {
                    text,
                    width,
                } => self.push_text(text, *width, TextBreaks::AsWritten),
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
                    self.push_text(" ", SPACE_WIDTH, TextBreaks::AsLineEnding);
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
                    elements: self.document.children(*elements),
                }),
                Element::Indent {
                    group,
                    elements,
                } => {
                    let indent = if group.is_none_or(|group| self.group_broke(group)) {
                        self.deeper(indent)
                    } else {
                        indent
                    };
                    self.push_elements(indent, mode, self.document.children(*elements));
                },
                Element::Align {
                    columns,
                    group,
                    elements,
                } => {
                    let indent = if group.is_none_or(|group| self.group_broke(group)) {
                        self.aligned(indent, *columns)
                    } else {
                        indent
                    };
                    self.push_elements(indent, mode, self.document.children(*elements));
                },
                Element::IfBreak(sides) => {
                    let (broken, flat) = self.document.sides(*sides);
                    let side = match mode {
                        Mode::Flat => flat,
                        Mode::Break => broken,
                    };
                    self.push_elements(indent, mode, side);
                },
                Element::Group {
                    id,
                    elements,
                } => {
                    let elements = self.document.children(*elements);
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

    /// Never fitting a forced break replaces a separate pass that marks the groups around it.
    fn fits(&self, elements: &'a [Element<'a>]) -> bool {
        let Some(mut remaining) = self.options.line_width.checked_sub(self.column) else {
            return false;
        };
        let mut has_line_suffix = !self.line_suffixes.is_empty();
        let rest = self
            .stack
            .iter()
            .rev()
            .map(|frame| (Measure::from(frame.mode), frame.element));
        let mut outer = elements
            .iter()
            .map(|element| (Measure::Flat, element))
            .chain(rest);
        let mut nested = Vec::new();
        loop {
            let next = next_nested(&mut nested).or_else(|| outer.next());
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
                    }
                    | Element::ExactText {
                        width: TextWidth::Multiline,
                        ..
                    },
                    Measure::Flat | Measure::InSuffix,
                ) => return false,
                (
                    Element::Group {
                        elements, ..
                    }
                    | Element::Indent {
                        elements, ..
                    }
                    | Element::Align {
                        elements, ..
                    },
                    _,
                ) => {
                    nested.push((measure, self.document.children(*elements).iter()));
                    0
                },
                (Element::IfBreak(sides), _) => {
                    let (broken, flat) = self.document.sides(*sides);
                    let side = match measure {
                        Measure::Break => broken,
                        Measure::Flat | Measure::InSuffix => flat,
                    };
                    nested.push((measure, side.iter()));
                    0
                },
                (Element::LineSuffix(elements), _) => {
                    has_line_suffix = true;
                    if measure != Measure::Break {
                        nested.push((Measure::InSuffix, self.document.children(*elements).iter()));
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
                ) => return columns(self.first_line(text, TextBreaks::AsLineEnding)) <= remaining,
                (
                    Element::ExactText {
                        width: TextWidth::Multiline,
                        text,
                    },
                    Measure::Break,
                ) => return columns(self.first_line(text, TextBreaks::AsWritten)) <= remaining,
                (
                    Element::Text {
                        width: TextWidth::Width(width),
                        ..
                    }
                    | Element::ExactText {
                        width: TextWidth::Width(width),
                        ..
                    },
                    _,
                ) => width.get(),
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

    fn flush_line_suffixes(&mut self, then: Option<Frame<'a>>) {
        if let Some(then) = then {
            self.stack.push(then);
        }
        let mut suffixes = mem::take(&mut self.line_suffixes);
        for suffix in suffixes.drain(..).rev() {
            self.push_elements(suffix.indent, suffix.mode, suffix.elements);
        }
        self.line_suffixes = suffixes;
    }

    fn push_elements(&mut self, indent: Indentation, mode: Mode, elements: &'a [Element<'a>]) {
        self.stack
            .extend(elements.iter().rev().map(|element| Frame {
                indent,
                mode,
                element,
            }));
    }

    fn group_broke(&self, group: GroupId) -> bool {
        self.group_modes.get(&group) == Some(&Mode::Break)
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

    fn push_text(&mut self, text: &str, width: TextWidth, breaks: TextBreaks) {
        // The printer trims only before breaks of its own.
        // An indent here would stay at the end of the line.
        let empty_first_line =
            width == TextWidth::Multiline && self.first_line(text, breaks).is_empty();
        if let Some(indent) = self.pending_indent.take().filter(|_| !empty_first_line) {
            self.out.extend(iter::repeat_n('\t', indent.tabs as usize));
            self.out.extend(iter::repeat_n(' ', indent.spaces as usize));
        }
        match width {
            TextWidth::Width(width) => {
                self.out.push_str(text);
                self.column = self.column.saturating_add(width.get());
            },
            TextWidth::Multiline => {
                let (before_last, last) = text
                    .rsplit_once('\n')
                    .expect("text over lines holds a line break");
                match breaks {
                    TextBreaks::AsLineEnding => {
                        let line_ending = self.options.line_ending;
                        for line in before_last.split('\n') {
                            self.out.push_str(line_ending.without_break(line));
                            self.out.push_str(line_ending.as_str());
                        }
                        self.out.push_str(last);
                    },
                    TextBreaks::AsWritten => self.out.push_str(text),
                }
                self.column = columns(last);
            },
        }
    }

    /// With the line broken as written, a `\r` right before the `\n` makes one break with it.
    fn first_line<'t>(&self, text: &'t str, breaks: TextBreaks) -> &'t str {
        text.split_once('\n')
            .map_or(text, |(first, _)| match breaks {
                TextBreaks::AsLineEnding => self.options.line_ending.without_break(first),
                TextBreaks::AsWritten => first.strip_suffix('\r').unwrap_or(first),
            })
    }

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

fn next_nested<'a>(
    nested: &mut Vec<(Measure, slice::Iter<'a, Element<'a>>)>,
) -> Option<(Measure, &'a Element<'a>)> {
    while let Some((measure, elements)) = nested.last_mut() {
        if let Some(element) = elements.next() {
            let measure = *measure;
            // Popped at once, so that lists nested last do not pile up their finished parents.
            if elements.as_slice().is_empty() {
                nested.pop();
            }
            return Some((measure, element));
        }
        nested.pop();
    }
    None
}

#[cfg(test)]
mod tests {
    use super::{IndentStyle, LineEnding, PrinterOptions, print};
    use crate::{
        Build, Builder, GroupIds, empty_line, empty_lines, exact_text, expand_parent,
        hard_line_break, line_suffix_boundary, soft_line_break, soft_line_break_or_space, space,
        text,
    };

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

    fn comment(b: &mut Builder<'static>) {
        b.line_suffix(|b| b.extend([space(), text("# c")]));
    }

    #[test]
    fn text_and_space_print_verbatim() {
        let doc = Builder::build(|b| b.extend([text("echo"), space(), text("a")]));
        assert_eq!(print(&doc, &WIDE), "echo a");
    }

    #[test]
    fn text_and_space_take_their_columns() {
        let doc = Builder::build(|b| {
            b.extend([text("a"), space()]);
            b.group(|b| b.extend([text("b"), soft_line_break(), text("c")]));
        });
        assert_eq!(print(&doc, &narrow(4)), "a bc");
        assert_eq!(print(&doc, &narrow(3)), "a b\nc");
    }

    #[test]
    fn empty_document_prints_nothing() {
        assert_eq!(print(&Builder::build(|_| {}), &WIDE), "");
    }

    #[test]
    fn hard_line_break_indents_next_text() {
        let doc = Builder::build(|b| {
            b.push(text("a"));
            b.indent(|b| {
                b.extend([hard_line_break(), text("b")]);
                b.indent(|b| b.extend([hard_line_break(), text("c")]));
            });
            b.extend([hard_line_break(), text("d")]);
        });
        assert_eq!(print(&doc, &WIDE), "a\n  b\n    c\nd");
        assert_eq!(print(&doc, &TABS), "a\n\tb\n\t\tc\nd");
    }

    #[test]
    fn indent_without_line_break_prints_nothing() {
        let doc = Builder::build(|b| b.indent(|b| b.push(text("a"))));
        assert_eq!(print(&doc, &WIDE), "a");
    }

    #[test]
    fn line_start_space_comes_after_indent() {
        let doc = Builder::build(|b| {
            b.push(text("a"));
            b.indent(|b| b.extend([hard_line_break(), space(), text("b")]));
        });
        assert_eq!(print(&doc, &WIDE), "a\n   b");
    }

    #[test]
    fn trailing_spaces_and_tabs_are_trimmed() {
        let doc = Builder::build(|b| {
            b.extend([
                text("a "),
                space(),
                hard_line_break(),
                text("b\t"),
                empty_line(),
                text("c"),
            ]);
        });
        assert_eq!(print(&doc, &WIDE), "a\nb\n\nc");
    }

    #[test]
    fn consecutive_line_breaks_collapse() {
        let doc = Builder::build(|b| {
            b.extend([
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
            ]);
        });
        assert_eq!(print(&doc, &WIDE), "a\nb\n\nc\n\nd\n\ne");
    }

    #[test]
    fn consecutive_line_breaks_collapse_with_crlf() {
        let doc = Builder::build(|b| {
            b.extend([
                text("a "),
                hard_line_break(),
                hard_line_break(),
                text("b"),
                empty_line(),
                empty_line(),
                text("c"),
            ]);
        });
        assert_eq!(print(&doc, &CRLF), "a\r\nb\r\n\r\nc");
    }

    #[test]
    fn consecutive_empty_lines_never_add_up() {
        let doc = Builder::build(|b| {
            b.extend([
                text("a"),
                empty_lines(2),
                empty_lines(2),
                text("b"),
                empty_line(),
                empty_lines(2),
                text("c"),
            ]);
        });
        assert_eq!(print(&doc, &WIDE), "a\n\n\nb\n\n\nc");
    }

    #[test]
    fn only_last_line_of_text_is_trimmed() {
        let doc = Builder::build(|b| b.extend([text("a \nb "), hard_line_break(), text("c")]));
        assert_eq!(print(&doc, &WIDE), "a \nb\nc");
    }

    #[test]
    fn only_lf_trims_cr_before_hard_line_break() {
        let doc = Builder::build(|b| b.extend([text("a \r\r"), hard_line_break(), text("b\r \r")]));
        assert_eq!(print(&doc, &WIDE), "a\nb\r \r");
        assert_eq!(print(&doc, &CRLF), "a \r\r\r\nb\r \r");
    }

    #[test]
    fn only_lf_trims_cr_before_line_break_in_text() {
        let doc = Builder::build(|b| b.push(text("a\r\r\nb \r\nc")));
        assert_eq!(print(&doc, &WIDE), "a\nb \nc");
        assert_eq!(print(&doc, &CRLF), "a\r\r\nb \r\nc");
        let doc = Builder::build(|b| {
            b.push(text("a"));
            b.indent(|b| b.extend([hard_line_break(), text("\r\r\nb")]));
        });
        assert_eq!(print(&doc, &WIDE), "a\n\nb");
        assert_eq!(print(&doc, &CRLF), "a\r\n  \r\r\nb");
    }

    #[test]
    fn only_untrimmed_cr_takes_width() {
        let doc = Builder::build(|b| {
            b.group(|b| b.extend([text("a"), soft_line_break_or_space(), text("b")]));
            b.push(text("x\r\r\ny"));
        });
        assert_eq!(print(&doc, &narrow(4)), "a bx\ny");
        let crlf = PrinterOptions {
            line_ending: LineEnding::CrLf,
            ..narrow(4)
        };
        assert_eq!(print(&doc, &crlf), "a\r\nbx\r\r\ny");
    }

    #[test]
    fn line_of_only_spaces_disappears() {
        let doc = Builder::build(|b| {
            b.extend([
                text("a"),
                hard_line_break(),
                space(),
                hard_line_break(),
                text("b"),
            ]);
        });
        assert_eq!(print(&doc, &WIDE), "a\nb");
    }

    #[test]
    fn output_never_starts_with_line_break() {
        let doc = Builder::build(|b| b.extend([hard_line_break(), empty_line(), text("a")]));
        assert_eq!(print(&doc, &WIDE), "a");
        let doc = Builder::build(|b| b.extend([space(), hard_line_break(), text("a")]));
        assert_eq!(print(&doc, &WIDE), "a");
        let doc = Builder::build(|b| b.indent(|b| b.extend([hard_line_break(), text("a")])));
        assert_eq!(print(&doc, &WIDE), "  a");
    }

    #[test]
    fn empty_lines_carry_no_indent() {
        let doc = Builder::build(|b| {
            b.push(text("a"));
            b.indent(|b| b.extend([empty_line(), text("b")]));
        });
        assert_eq!(print(&doc, &WIDE), "a\n\n  b");
    }

    #[test]
    fn final_line_break_leaves_no_indent_behind() {
        let doc = Builder::build(|b| b.indent(|b| b.extend([text("a"), hard_line_break()])));
        assert_eq!(print(&doc, &WIDE), "a\n");
    }

    #[test]
    fn line_ending_sets_line_break_characters() {
        let doc = Builder::build(|b| {
            b.push(text("a"));
            b.indent(|b| b.extend([hard_line_break(), text("b")]));
        });
        assert_eq!(print(&doc, &CRLF), "a\r\n  b");
    }

    #[test]
    fn indent_width_sets_spaces_per_level() {
        let doc = Builder::build(|b| {
            b.push(text("a"));
            b.indent(|b| b.extend([hard_line_break(), text("b")]));
        });
        let four_column_indent = PrinterOptions {
            indent_width: 4,
            ..WIDE
        };
        assert_eq!(print(&doc, &four_column_indent), "a\n    b");
    }

    #[test]
    fn group_stays_flat_up_to_line_width() {
        let doc = Builder::build(|b| {
            b.group(|b| {
                b.extend([
                    text("aa"),
                    soft_line_break_or_space(),
                    text("bb"),
                    soft_line_break(),
                    text("cc"),
                ]);
            });
        });
        assert_eq!(print(&doc, &narrow(7)), "aa bbcc");
        assert_eq!(print(&doc, &narrow(6)), "aa\nbb\ncc");
    }

    #[test]
    fn if_break_prints_side_of_mode_around_it() {
        let doc = Builder::build(|b| {
            b.group(|b| {
                b.push(text("a"));
                b.if_break(|b| b.push(text("!")), |b| b.push(text("?")));
                b.extend([soft_line_break_or_space(), text("b")]);
            });
        });
        assert_eq!(print(&doc, &WIDE), "a? b");
        assert_eq!(print(&doc, &narrow(3)), "a!\nb");
        let outside_group =
            Builder::build(|b| b.if_break(|b| b.push(text("!")), |b| b.push(text("?"))));
        assert_eq!(print(&outside_group, &WIDE), "!");
    }

    #[test]
    fn group_measures_flat_side_of_if_break() {
        let doc = Builder::build(|b| {
            b.group(|b| {
                b.push(text("a"));
                b.if_break(|b| b.push(text("xxxx")), |b| b.push(text("bbb")));
                b.extend([soft_line_break(), text("c")]);
            });
        });
        assert_eq!(print(&doc, &narrow(5)), "abbbc");
        assert_eq!(print(&doc, &narrow(4)), "axxxx\nc");
    }

    #[test]
    fn rest_of_line_measures_broken_side_of_if_break() {
        let doc = Builder::build(|b| {
            b.group(|b| b.extend([text("a"), soft_line_break(), text("b")]));
            b.if_break(|b| b.push(text("ccc")), |b| b.push(text("dddd")));
        });
        assert_eq!(print(&doc, &narrow(5)), "abccc");
        assert_eq!(print(&doc, &narrow(4)), "a\nbccc");
    }

    #[test]
    fn if_break_prints_containers_inside_its_sides() {
        let doc = Builder::build(|b| {
            b.group(|b| {
                b.push(text("x"));
                b.if_break(
                    |b| b.indent(|b| b.extend([hard_line_break(), text("a")])),
                    |b| b.group(|b| b.extend([soft_line_break(), text("b")])),
                );
                b.extend([soft_line_break(), text("y")]);
            });
        });
        assert_eq!(print(&doc, &WIDE), "xby");
        assert_eq!(print(&doc, &narrow(2)), "x\n  a\ny");
    }

    #[test]
    fn line_suffix_measures_flat_side_of_if_break() {
        let breaks_when_broken = Builder::build(|b| {
            b.group(|b| {
                b.push(text("a"));
                b.line_suffix(|b| {
                    b.if_break(|b| b.push(hard_line_break()), |b| b.push(text("!")));
                });
                b.extend([soft_line_break(), text("c")]);
            });
        });
        assert_eq!(print(&breaks_when_broken, &WIDE), "ac!");
        let breaks_when_flat = Builder::build(|b| {
            b.group(|b| {
                b.push(text("a"));
                b.line_suffix(|b| {
                    b.if_break(|b| b.push(text("!")), |b| b.push(hard_line_break()));
                });
                b.extend([soft_line_break(), text("c")]);
            });
        });
        assert_eq!(print(&breaks_when_flat, &WIDE), "a!\nc");
    }

    #[test]
    fn group_starting_past_line_width_breaks() {
        let doc = Builder::build(|b| {
            b.push(text("aaaa"));
            b.group(|b| b.extend([soft_line_break(), text("b")]));
        });
        assert_eq!(print(&doc, &narrow(3)), "aaaa\nb");
    }

    #[test]
    fn soft_lines_outside_group_break() {
        let doc = Builder::build(|b| b.extend([text("a"), soft_line_break_or_space(), text("b")]));
        assert_eq!(print(&doc, &WIDE), "a\nb");
    }

    #[test]
    fn group_inside_flat_group_stays_flat() {
        let doc = Builder::build(|b| {
            b.group(|b| {
                b.extend([text("a"), soft_line_break_or_space()]);
                b.group(|b| b.extend([text("b"), soft_line_break(), text("c")]));
            });
        });
        assert_eq!(print(&doc, &WIDE), "a bc");
    }

    #[test]
    fn forced_break_breaks_enclosing_groups() {
        let hard = Builder::build(|b| {
            b.group(|b| {
                b.extend([text("a"), soft_line_break_or_space()]);
                b.group(|b| b.extend([text("b"), soft_line_break_or_space(), text("c")]));
                b.push(soft_line_break_or_space());
                b.group(|b| b.extend([text("d"), hard_line_break(), text("e")]));
            });
        });
        assert_eq!(print(&hard, &WIDE), "a\nb c\nd\ne");
        let empty = Builder::build(|b| b.group(|b| b.extend([text("a"), empty_line(), text("b")])));
        assert_eq!(print(&empty, &WIDE), "a\n\nb");
        let expand = Builder::build(|b| {
            b.group(|b| {
                b.extend([text("a"), soft_line_break_or_space()]);
                b.group(|b| b.extend([text("b"), expand_parent(), text("c")]));
            });
        });
        assert_eq!(print(&expand, &WIDE), "a\nbc");
        let multiline = Builder::build(|b| {
            b.group(|b| b.extend([text("a\nb"), soft_line_break_or_space(), text("c")]));
        });
        assert_eq!(print(&multiline, &WIDE), "a\nb\nc");
        let in_indent = Builder::build(|b| {
            b.group(|b| {
                b.push(text("a"));
                b.indent(|b| {
                    b.extend([
                        soft_line_break_or_space(),
                        text("b"),
                        hard_line_break(),
                        text("c"),
                    ]);
                });
            });
        });
        assert_eq!(print(&in_indent, &WIDE), "a\n  b\n  c");
    }

    #[test]
    fn forced_break_in_line_suffix_breaks_enclosing_group() {
        let forced_breaks: [fn(&mut Builder<'static>); 6] = [
            |b| b.push(expand_parent()),
            |b| b.push(hard_line_break()),
            |b| b.push(text("\n")),
            |b| b.push(exact_text("\n")),
            |b| b.indent(|b| b.group(|b| b.push(hard_line_break()))),
            |b| b.line_suffix(|b| b.push(hard_line_break())),
        ];
        for (index, forced_break) in forced_breaks.into_iter().enumerate() {
            let doc = Builder::build(|b| {
                b.group(|b| {
                    b.push(text("a"));
                    b.line_suffix(forced_break);
                    b.extend([soft_line_break_or_space(), text("b")]);
                });
            });
            assert_eq!(print(&doc, &WIDE), "a\nb", "forced break {index}");
        }
    }

    #[test]
    fn boundary_in_line_suffix_never_ends_measurement() {
        let doc = Builder::build(|b| {
            b.group(|b| {
                b.push(text("a"));
                b.line_suffix(|b| b.extend([line_suffix_boundary(), hard_line_break()]));
                b.extend([soft_line_break_or_space(), text("b")]);
            });
        });
        assert_eq!(print(&doc, &WIDE), "a\nb");
    }

    #[test]
    fn group_ignores_forced_break_in_following_line_suffix() {
        let doc = Builder::build(|b| {
            b.group(|b| b.extend([text("a"), soft_line_break_or_space(), text("b")]));
            b.line_suffix(|b| b.extend([hard_line_break(), text("# c")]));
        });
        assert_eq!(print(&doc, &WIDE), "a b\n# c");
    }

    #[test]
    fn groups_inside_broken_group_fit_independently() {
        let doc = Builder::build(|b| {
            b.group(|b| {
                b.extend([text("call"), soft_line_break_or_space()]);
                b.group(|b| b.extend([text("a"), soft_line_break_or_space(), text("b")]));
                b.push(soft_line_break_or_space());
                b.group(|b| b.extend([text("c"), soft_line_break_or_space(), text("d")]));
            });
        });
        assert_eq!(print(&doc, &narrow(6)), "call\na b\nc d");
    }

    #[test]
    fn rest_of_line_counts_toward_group_fit() {
        let group_then = |rest: fn(&mut Builder<'static>)| {
            Builder::build(|b| {
                b.group(|b| b.extend([text("a"), soft_line_break_or_space(), text("b")]));
                rest(b);
            })
        };
        assert_eq!(
            print(&group_then(|b| b.push(text(";"))), &narrow(4)),
            "a b;"
        );
        assert_eq!(
            print(&group_then(|b| b.push(text(";;"))), &narrow(4)),
            "a\nb;;"
        );
        let then_line = group_then(|b| b.extend([hard_line_break(), text("long line")]));
        assert_eq!(print(&then_line, &narrow(4)), "a b\nlong line");
        let then_text = group_then(|b| b.push(text("x\r\ny")));
        assert_eq!(print(&then_text, &narrow(4)), "a bx\ny");
        let then_suffix = group_then(|b| b.line_suffix(|b| b.push(text("# a comment"))));
        assert_eq!(print(&then_suffix, &narrow(4)), "a b# a comment");
    }

    #[test]
    fn indent_takes_columns() {
        let doc = Builder::build(|b| {
            b.push(text("a"));
            b.indent(|b| {
                b.push(hard_line_break());
                b.group(|b| b.extend([text("aa"), soft_line_break_or_space(), text("bb")]));
            });
        });
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
        let doc =
            Builder::build(|b| b.extend([text("a\r\nb\nc\rd"), hard_line_break(), text("e")]));
        assert_eq!(print(&doc, &WIDE), "a\nb\nc\rd\ne");
        assert_eq!(print(&doc, &CRLF), "a\r\nb\r\nc\rd\r\ne");
    }

    #[test]
    fn exact_text_keeps_its_line_breaks() {
        let doc = Builder::build(|b| {
            b.extend([exact_text("a\r\nb\nc\r\r\nd"), hard_line_break(), text("e")]);
        });
        assert_eq!(print(&doc, &WIDE), "a\r\nb\nc\r\r\nd\ne");
        assert_eq!(print(&doc, &CRLF), "a\r\nb\nc\r\r\nd\r\ne");
    }

    #[test]
    fn cr_before_line_break_in_exact_text_takes_no_width() {
        let doc = Builder::build(|b| {
            b.group(|b| b.extend([text("a"), soft_line_break_or_space(), text("b")]));
            b.push(exact_text("x\r\ny"));
        });
        assert_eq!(print(&doc, &narrow(4)), "a bx\r\ny");
        let doc = Builder::build(|b| {
            b.group(|b| b.extend([text("a"), soft_line_break_or_space(), text("b")]));
            b.push(exact_text("x\r\r\ny"));
        });
        assert_eq!(print(&doc, &narrow(4)), "a\nbx\r\r\ny");
    }

    #[test]
    fn exact_text_over_lines_breaks_enclosing_group() {
        let doc = Builder::build(|b| {
            b.group(|b| b.extend([text("a"), soft_line_break_or_space(), exact_text("b\nc")]));
        });
        assert_eq!(print(&doc, &WIDE), "a\nb\nc");
    }

    #[test]
    fn exact_text_resets_column_after_its_last_line_break() {
        let doc = Builder::build(|b| {
            b.push(exact_text("a\r\nbb"));
            b.group(|b| b.extend([text("c"), soft_line_break_or_space(), text("d")]));
        });
        assert_eq!(print(&doc, &narrow(5)), "a\r\nbbc d");
        assert_eq!(print(&doc, &narrow(4)), "a\r\nbbc\nd");
    }

    #[test]
    fn line_break_ending_text_collapses_with_hard_line_break() {
        let doc = Builder::build(|b| b.extend([text("a\n"), hard_line_break(), text("b")]));
        assert_eq!(print(&doc, &WIDE), "a\nb");
        assert_eq!(print(&doc, &CRLF), "a\r\nb");
    }

    #[test]
    fn only_first_line_of_text_is_indented() {
        let doc = Builder::build(|b| {
            b.push(text("a"));
            b.indent(|b| b.extend([hard_line_break(), text("b\nc")]));
        });
        assert_eq!(print(&doc, &WIDE), "a\n  b\nc");
    }

    #[test]
    fn empty_first_line_of_text_carries_no_indent() {
        for multiline in ["\nb", "\r\nb"] {
            let doc = Builder::build(|b| {
                b.push(text("a"));
                b.indent(|b| b.extend([hard_line_break(), text(multiline)]));
            });
            assert_eq!(print(&doc, &WIDE), "a\n\nb", "{multiline:?}");
            assert_eq!(print(&doc, &TABS), "a\n\nb", "{multiline:?}");
        }
        for multiline in ["\nb", "\r\nb"] {
            let doc = Builder::build(|b| {
                b.push(text("a"));
                b.indent(|b| b.extend([hard_line_break(), exact_text(multiline)]));
            });
            assert_eq!(
                print(&doc, &WIDE),
                format!("a\n{multiline}"),
                "{multiline:?}"
            );
        }
    }

    #[test]
    fn empty_text_leaves_indent_to_next_text() {
        let doc = Builder::build(|b| {
            b.push(text("a"));
            b.indent(|b| b.extend([hard_line_break(), text(""), text("b")]));
        });
        assert_eq!(print(&doc, &WIDE), "a\n  b");
    }

    #[test]
    fn multiline_text_resets_column() {
        let doc = Builder::build(|b| {
            b.push(text("a\nbb"));
            b.group(|b| b.extend([text("c"), soft_line_break_or_space(), text("d")]));
        });
        assert_eq!(print(&doc, &narrow(5)), "a\nbbc d");
    }

    #[test]
    fn line_suffix_keeps_written_indent_and_mode() {
        let doc = Builder::build(|b| {
            b.push(text("a"));
            b.indent(|b| b.line_suffix(|b| b.extend([hard_line_break(), text("# c")])));
            b.extend([hard_line_break(), text("b")]);
        });
        assert_eq!(print(&doc, &WIDE), "a\n  # c\nb");
        let doc = Builder::build(|b| {
            b.group(|b| {
                b.push(text("a"));
                b.line_suffix(|b| b.extend([space(), text("# c"), soft_line_break(), text("!")]));
                b.extend([space(), text("b")]);
            });
            b.extend([hard_line_break(), text("d")]);
        });
        assert_eq!(print(&doc, &WIDE), "a b # c!\nd");
    }

    #[test]
    fn boundary_in_flat_group_keeps_line_suffix_group_flat() {
        let doc = Builder::build(|b| {
            b.push(text("a"));
            b.line_suffix(|b| {
                b.group(|b| b.extend([text(" #"), soft_line_break_or_space(), text("c")]));
            });
            b.group(|b| b.extend([line_suffix_boundary(), text("b")]));
        });
        assert_eq!(print(&doc, &WIDE), "a # c\nb");
    }

    #[test]
    fn line_suffix_waits_for_next_line_break() {
        let doc = Builder::build(|b| {
            b.push(text("a"));
            comment(b);
            b.extend([space(), text("b"), hard_line_break(), text("c")]);
        });
        assert_eq!(print(&doc, &WIDE), "a b # c\nc");
        let doc = Builder::build(|b| {
            b.push(text("a"));
            comment(b);
            b.extend([empty_line(), text("b")]);
        });
        assert_eq!(print(&doc, &WIDE), "a # c\n\nb");
        let doc = Builder::build(|b| {
            b.group(|b| {
                b.push(text("a"));
                comment(b);
                b.extend([soft_line_break_or_space(), text("b")]);
            });
        });
        assert_eq!(print(&doc, &WIDE), "a b # c");
        assert_eq!(print(&doc, &narrow(2)), "a # c\nb");
    }

    #[test]
    fn line_suffixes_keep_their_order() {
        let doc = Builder::build(|b| {
            b.push(text("a"));
            b.line_suffix(|b| b.push(text(" x")));
            b.line_suffix(|b| b.push(text(" y")));
            b.extend([hard_line_break(), text("b")]);
        });
        assert_eq!(print(&doc, &WIDE), "a x y\nb");
    }

    #[test]
    fn line_suffix_at_end_still_prints() {
        let doc = Builder::build(|b| {
            b.push(text("a"));
            comment(b);
        });
        assert_eq!(print(&doc, &WIDE), "a # c");
    }

    #[test]
    fn boundary_breaks_line_only_after_line_suffix() {
        let doc = Builder::build(|b| {
            b.push(text("a"));
            comment(b);
            b.extend([line_suffix_boundary(), text("b")]);
        });
        assert_eq!(print(&doc, &WIDE), "a # c\nb");
        let doc = Builder::build(|b| b.extend([text("a"), line_suffix_boundary(), text("b")]));
        assert_eq!(print(&doc, &WIDE), "ab");
    }

    #[test]
    fn line_suffix_takes_no_width() {
        let doc = Builder::build(|b| {
            b.group(|b| {
                b.push(text("aa"));
                b.line_suffix(|b| b.push(text(" # a long comment")));
                b.extend([soft_line_break_or_space(), text("bb")]);
            });
        });
        assert_eq!(print(&doc, &narrow(5)), "aa bb # a long comment");
    }

    #[test]
    fn boundary_after_line_suffix_ends_measurement() {
        let doc = Builder::build(|b| {
            b.push(text("aaa"));
            comment(b);
            b.group(|b| {
                b.extend([
                    line_suffix_boundary(),
                    text("bbb"),
                    soft_line_break_or_space(),
                    text("ccc"),
                ]);
            });
        });
        assert_eq!(print(&doc, &narrow(7)), "aaa # c\nbbb ccc");
    }

    #[test]
    fn alignment_adds_its_columns_to_indent() {
        let doc = Builder::build(|b| {
            b.push(text("a"));
            b.align(2, |b| {
                b.extend([hard_line_break(), text("b")]);
                b.indent(|b| b.extend([hard_line_break(), text("c")]));
            });
            b.indent(|b| b.align(1, |b| b.extend([hard_line_break(), text("d")])));
        });
        assert_eq!(print(&doc, &WIDE), "a\n  b\n    c\n   d");
    }

    #[test]
    fn alignment_before_indent_level_becomes_tab() {
        let doc = Builder::build(|b| {
            b.push(text("a"));
            b.align(2, |b| {
                b.extend([hard_line_break(), text("b")]);
                b.indent(|b| {
                    b.extend([hard_line_break(), text("c")]);
                    b.align(1, |b| b.extend([hard_line_break(), text("d")]));
                });
            });
        });
        assert_eq!(print(&doc, &TABS), "a\n  b\n\t\tc\n\t\t d");
    }

    #[test]
    fn alignment_on_group_follows_whether_it_broke() {
        let id = GroupIds::default().fresh();
        let doc = Builder::build(|b| {
            b.group_with_id(id, |b| {
                b.extend([text("aaa"), soft_line_break_or_space(), text("|")]);
            });
            b.align_if_group_breaks(id, 2, |b| {
                b.extend([space(), text("b"), hard_line_break(), text("c")]);
            });
        });
        assert_eq!(print(&doc, &WIDE), "aaa | b\nc");
        assert_eq!(print(&doc, &narrow(4)), "aaa\n| b\n  c");
    }

    #[test]
    fn indent_on_group_follows_whether_it_broke() {
        let id = GroupIds::default().fresh();
        let doc = Builder::build(|b| {
            b.group_with_id(id, |b| {
                b.push(text("aaa"));
                b.indent_if_group_breaks(id, |b| {
                    b.extend([soft_line_break_or_space(), text("| b")]);
                });
            });
            b.indent_if_group_breaks(id, |b| {
                b.extend([hard_line_break(), text("c")]);
            });
        });
        assert_eq!(print(&doc, &WIDE), "aaa | b\nc");
        assert_eq!(print(&doc, &narrow(4)), "aaa\n  | b\n  c");
    }

    #[test]
    fn indent_levels_on_group_take_one_indent_per_level() {
        let id = GroupIds::default().fresh();
        let doc = Builder::build(|b| {
            b.group_with_id(id, |b| {
                b.extend([text("a"), soft_line_break(), text("b")]);
            });
            b.indent_levels_if_group_breaks(id, 2, |b| {
                b.extend([hard_line_break(), text("c")]);
            });
        });
        assert_eq!(print(&doc, &WIDE), "ab\nc");
        assert_eq!(print(&doc, &narrow(1)), "a\nb\n    c");
    }

    #[test]
    fn indent_on_group_needs_it_already_broken() {
        let id = GroupIds::default().fresh();
        let in_flat_parent = Builder::build(|b| {
            b.group(|b| {
                b.group_with_id(id, |b| {
                    b.extend([text("x"), soft_line_break(), text("y")]);
                });
            });
            b.indent_if_group_breaks(id, |b| {
                b.extend([hard_line_break(), text("b")]);
            });
        });
        assert_eq!(print(&in_flat_parent, &WIDE), "xy\nb");
        let unprinted = Builder::build(|b| {
            b.indent_if_group_breaks(id, |b| {
                b.extend([text("a"), hard_line_break(), text("b")]);
            });
            b.group_with_id(id, |b| {
                b.extend([text("c"), hard_line_break(), text("d")]);
            });
        });
        assert_eq!(print(&unprinted, &WIDE), "a\nbc\nd");
    }

    #[test]
    fn alignment_takes_columns() {
        let doc = Builder::build(|b| {
            b.push(text("a"));
            b.align(3, |b| {
                b.push(hard_line_break());
                b.group(|b| b.extend([text("aa"), soft_line_break_or_space(), text("bb")]));
            });
        });
        assert_eq!(print(&doc, &narrow(8)), "a\n   aa bb");
        assert_eq!(print(&doc, &narrow(7)), "a\n   aa\n   bb");
    }

    #[test]
    fn alignment_takes_columns_after_tabs_too() {
        let doc = Builder::build(|b| {
            b.push(text("a"));
            b.indent(|b| {
                b.align(2, |b| {
                    b.push(hard_line_break());
                    b.group(|b| b.extend([text("bb"), soft_line_break_or_space(), text("cc")]));
                });
            });
        });
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
        let doc = Builder::build(|b| {
            b.push(text("a"));
            b.align(0, |b| {
                b.indent(|b| b.extend([hard_line_break(), text("b")]));
            });
        });
        assert_eq!(print(&doc, &TABS), "a\n\tb");
    }

    #[test]
    fn alignment_on_group_needs_it_already_broken() {
        let id = GroupIds::default().fresh();
        let flat = Builder::build(|b| {
            b.group(|b| {
                b.group_with_id(id, |b| b.push(text("a")));
                b.align_if_group_breaks(id, 2, |b| {
                    b.extend([hard_line_break(), text("b")]);
                });
            });
        });
        assert_eq!(print(&flat, &WIDE), "a\nb");
        let in_flat_parent = Builder::build(|b| {
            b.group(|b| {
                b.group_with_id(id, |b| {
                    b.extend([text("x"), soft_line_break(), text("y")]);
                });
            });
            b.align_if_group_breaks(id, 2, |b| {
                b.extend([hard_line_break(), text("b")]);
            });
        });
        assert_eq!(print(&in_flat_parent, &WIDE), "xy\nb");
        let unprinted = Builder::build(|b| {
            b.align_if_group_breaks(id, 2, |b| {
                b.extend([text("a"), hard_line_break(), text("b")]);
            });
            b.group_with_id(id, |b| {
                b.extend([text("c"), hard_line_break(), text("d")]);
            });
        });
        assert_eq!(print(&unprinted, &WIDE), "a\nbc\nd");
        let inside_own = Builder::build(|b| {
            b.group_with_id(id, |b| {
                b.extend([text("xx"), soft_line_break_or_space(), text("yy")]);
                b.align_if_group_breaks(id, 2, |b| {
                    b.extend([hard_line_break(), text("z")]);
                });
            });
        });
        assert_eq!(print(&inside_own, &WIDE), "xx\nyy\n  z");
    }

    #[test]
    fn alignment_spares_empty_lines_not_line_suffixes() {
        let doc = Builder::build(|b| {
            b.push(text("a"));
            b.align(2, |b| {
                b.extend([empty_line(), text("b")]);
                b.line_suffix(|b| b.extend([hard_line_break(), text("# c")]));
            });
            b.extend([hard_line_break(), text("d")]);
        });
        assert_eq!(print(&doc, &WIDE), "a\n\n  b\n  # c\nd");
    }
}
