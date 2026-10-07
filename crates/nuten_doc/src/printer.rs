use std::{collections::HashMap, iter, mem, slice};

use crate::{Document, Element, GroupId, LineMode, TextWidth, element::columns};

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

pub fn print(
    document: &Document<'_>,
    elements: &[Element<'_>],
    options: &PrinterOptions,
) -> String {
    let mut printer = Printer {
        options,
        document,
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
                    self.push_text(" ", TextWidth::Width(1), TextBreaks::AsLineEnding);
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
                Element::IfBreak {
                    broken,
                    flat,
                } => {
                    let elements = match mode {
                        Mode::Flat => flat,
                        Mode::Break => broken,
                    };
                    self.push_elements(indent, mode, self.document.children(*elements));
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
                (
                    Element::IfBreak {
                        broken, ..
                    },
                    Measure::Break,
                ) => {
                    nested.push((measure, self.document.children(*broken).iter()));
                    0
                },
                (
                    Element::IfBreak {
                        flat, ..
                    },
                    Measure::Flat | Measure::InSuffix,
                ) => {
                    nested.push((measure, self.document.children(*flat).iter()));
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
                self.column = self.column.saturating_add(width);
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
        Document, Element, GroupId, empty_line, empty_lines, exact_text, expand_parent,
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

    fn comment(doc: &mut Document<'static>) -> Element<'static> {
        doc.line_suffix([space(), text("# c")])
    }

    /// For elements that hold no children.
    fn print_leaves(elements: &[Element<'_>], options: &PrinterOptions) -> String {
        print(&Document::default(), elements, options)
    }

    #[test]
    fn text_and_space_print_verbatim() {
        let elements = [text("echo"), space(), text("a")];
        assert_eq!(print_leaves(&elements, &WIDE), "echo a");
    }

    #[test]
    fn hard_line_break_indents_next_text() {
        let mut doc = Document::default();
        let inner = doc.indent([hard_line_break(), text("c")]);
        let outer = doc.indent([hard_line_break(), text("b"), inner]);
        let elements = [text("a"), outer, hard_line_break(), text("d")];
        assert_eq!(print(&doc, &elements, &WIDE), "a\n  b\n    c\nd");
        assert_eq!(print(&doc, &elements, &TABS), "a\n\tb\n\t\tc\nd");
    }

    #[test]
    fn indent_without_line_break_prints_nothing() {
        let mut doc = Document::default();
        let elements = [doc.indent([text("a")])];
        assert_eq!(print(&doc, &elements, &WIDE), "a");
    }

    #[test]
    fn line_start_space_comes_after_indent() {
        let mut doc = Document::default();
        let elements = [
            text("a"),
            doc.indent([hard_line_break(), space(), text("b")]),
        ];
        assert_eq!(print(&doc, &elements, &WIDE), "a\n   b");
    }

    #[test]
    fn trailing_spaces_and_tabs_are_trimmed() {
        let elements = [
            text("a "),
            space(),
            hard_line_break(),
            text("b\t"),
            empty_line(),
            text("c"),
        ];
        assert_eq!(print_leaves(&elements, &WIDE), "a\nb\n\nc");
    }

    #[test]
    fn consecutive_line_breaks_collapse() {
        let elements = [
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
        assert_eq!(print_leaves(&elements, &WIDE), "a\nb\n\nc\n\nd\n\ne");
    }

    #[test]
    fn consecutive_line_breaks_collapse_with_crlf() {
        let elements = [
            text("a "),
            hard_line_break(),
            hard_line_break(),
            text("b"),
            empty_line(),
            empty_line(),
            text("c"),
        ];
        assert_eq!(print_leaves(&elements, &CRLF), "a\r\nb\r\n\r\nc");
    }

    #[test]
    fn consecutive_empty_lines_never_add_up() {
        let elements = [
            text("a"),
            empty_lines(2),
            empty_lines(2),
            text("b"),
            empty_line(),
            empty_lines(2),
            text("c"),
        ];
        assert_eq!(print_leaves(&elements, &WIDE), "a\n\n\nb\n\n\nc");
    }

    #[test]
    fn only_last_line_of_text_is_trimmed() {
        let elements = [text("a \nb "), hard_line_break(), text("c")];
        assert_eq!(print_leaves(&elements, &WIDE), "a \nb\nc");
    }

    #[test]
    fn only_lf_trims_cr_before_hard_line_break() {
        let elements = [text("a \r\r"), hard_line_break(), text("b\r \r")];
        assert_eq!(print_leaves(&elements, &WIDE), "a\nb\r \r");
        assert_eq!(print_leaves(&elements, &CRLF), "a \r\r\r\nb\r \r");
    }

    #[test]
    fn only_lf_trims_cr_before_line_break_in_text() {
        let elements = [text("a\r\r\nb \r\nc")];
        assert_eq!(print_leaves(&elements, &WIDE), "a\nb \nc");
        assert_eq!(print_leaves(&elements, &CRLF), "a\r\r\nb \r\nc");
        let mut doc = Document::default();
        let elements = [text("a"), doc.indent([hard_line_break(), text("\r\r\nb")])];
        assert_eq!(print(&doc, &elements, &WIDE), "a\n\nb");
        assert_eq!(print(&doc, &elements, &CRLF), "a\r\n  \r\r\nb");
    }

    #[test]
    fn only_untrimmed_cr_takes_width() {
        let mut doc = Document::default();
        let elements = [
            doc.group([text("a"), soft_line_break_or_space(), text("b")]),
            text("x\r\r\ny"),
        ];
        assert_eq!(print(&doc, &elements, &narrow(4)), "a bx\ny");
        let crlf = PrinterOptions {
            line_ending: LineEnding::CrLf,
            ..narrow(4)
        };
        assert_eq!(print(&doc, &elements, &crlf), "a\r\nbx\r\r\ny");
    }

    #[test]
    fn line_of_only_spaces_disappears() {
        let elements = [
            text("a"),
            hard_line_break(),
            space(),
            hard_line_break(),
            text("b"),
        ];
        assert_eq!(print_leaves(&elements, &WIDE), "a\nb");
    }

    #[test]
    fn output_never_starts_with_line_break() {
        let elements = [hard_line_break(), empty_line(), text("a")];
        assert_eq!(print_leaves(&elements, &WIDE), "a");
        let elements = [space(), hard_line_break(), text("a")];
        assert_eq!(print_leaves(&elements, &WIDE), "a");
        let mut doc = Document::default();
        let elements = [doc.indent([hard_line_break(), text("a")])];
        assert_eq!(print(&doc, &elements, &WIDE), "  a");
    }

    #[test]
    fn empty_lines_carry_no_indent() {
        let mut doc = Document::default();
        let elements = [text("a"), doc.indent([empty_line(), text("b")])];
        assert_eq!(print(&doc, &elements, &WIDE), "a\n\n  b");
    }

    #[test]
    fn final_line_break_leaves_no_indent_behind() {
        let mut doc = Document::default();
        let elements = [doc.indent([text("a"), hard_line_break()])];
        assert_eq!(print(&doc, &elements, &WIDE), "a\n");
    }

    #[test]
    fn line_ending_sets_line_break_characters() {
        let mut doc = Document::default();
        let elements = [text("a"), doc.indent([hard_line_break(), text("b")])];
        assert_eq!(print(&doc, &elements, &CRLF), "a\r\n  b");
    }

    #[test]
    fn indent_width_sets_spaces_per_level() {
        let mut doc = Document::default();
        let elements = [text("a"), doc.indent([hard_line_break(), text("b")])];
        let four_column_indent = PrinterOptions {
            indent_width: 4,
            ..WIDE
        };
        assert_eq!(print(&doc, &elements, &four_column_indent), "a\n    b");
    }

    #[test]
    fn group_stays_flat_up_to_line_width() {
        let mut doc = Document::default();
        let elements = [doc.group([
            text("aa"),
            soft_line_break_or_space(),
            text("bb"),
            soft_line_break(),
            text("cc"),
        ])];
        assert_eq!(print(&doc, &elements, &narrow(7)), "aa bbcc");
        assert_eq!(print(&doc, &elements, &narrow(6)), "aa\nbb\ncc");
    }

    #[test]
    fn if_break_prints_side_of_mode_around_it() {
        let mut doc = Document::default();
        let if_break = doc.if_break([text("!")], [text("?")]);
        let elements = [doc.group([text("a"), if_break, soft_line_break_or_space(), text("b")])];
        assert_eq!(print(&doc, &elements, &WIDE), "a? b");
        assert_eq!(print(&doc, &elements, &narrow(3)), "a!\nb");
        let outside_group = [if_break];
        assert_eq!(print(&doc, &outside_group, &WIDE), "!");
    }

    #[test]
    fn group_measures_flat_side_of_if_break() {
        let mut doc = Document::default();
        let if_break = doc.if_break([], [text("bbb")]);
        let elements = [doc.group([text("a"), if_break, soft_line_break(), text("c")])];
        assert_eq!(print(&doc, &elements, &narrow(5)), "abbbc");
        assert_eq!(print(&doc, &elements, &narrow(4)), "a\nc");
    }

    #[test]
    fn rest_of_line_measures_broken_side_of_if_break() {
        let mut doc = Document::default();
        let elements = [
            doc.group([text("a"), soft_line_break(), text("b")]),
            doc.if_break([text("ccc")], []),
        ];
        assert_eq!(print(&doc, &elements, &narrow(5)), "abccc");
        assert_eq!(print(&doc, &elements, &narrow(4)), "a\nbccc");
    }

    #[test]
    fn line_suffix_measures_flat_side_of_if_break() {
        let mut doc = Document::default();
        let if_break = doc.if_break([hard_line_break()], [text("!")]);
        let suffix = doc.line_suffix([if_break]);
        let breaks_when_broken = [doc.group([text("a"), suffix, soft_line_break(), text("c")])];
        assert_eq!(print(&doc, &breaks_when_broken, &WIDE), "ac!");
        let if_break = doc.if_break([text("!")], [hard_line_break()]);
        let suffix = doc.line_suffix([if_break]);
        let breaks_when_flat = [doc.group([text("a"), suffix, soft_line_break(), text("c")])];
        assert_eq!(print(&doc, &breaks_when_flat, &WIDE), "a!\nc");
    }

    #[test]
    fn group_starting_past_line_width_breaks() {
        let mut doc = Document::default();
        let elements = [text("aaaa"), doc.group([soft_line_break(), text("b")])];
        assert_eq!(print(&doc, &elements, &narrow(3)), "aaaa\nb");
    }

    #[test]
    fn soft_lines_outside_group_break() {
        let elements = [text("a"), soft_line_break_or_space(), text("b")];
        assert_eq!(print_leaves(&elements, &WIDE), "a\nb");
    }

    #[test]
    fn group_inside_flat_group_stays_flat() {
        let mut doc = Document::default();
        let inner = doc.group([text("b"), soft_line_break(), text("c")]);
        let elements = [doc.group([text("a"), soft_line_break_or_space(), inner])];
        assert_eq!(print(&doc, &elements, &WIDE), "a bc");
    }

    #[test]
    fn forced_break_breaks_enclosing_groups() {
        let mut doc = Document::default();
        let flat = doc.group([text("b"), soft_line_break_or_space(), text("c")]);
        let broken = doc.group([text("d"), hard_line_break(), text("e")]);
        let hard = [doc.group([
            text("a"),
            soft_line_break_or_space(),
            flat,
            soft_line_break_or_space(),
            broken,
        ])];
        assert_eq!(print(&doc, &hard, &WIDE), "a\nb c\nd\ne");
        let empty = [doc.group([text("a"), empty_line(), text("b")])];
        assert_eq!(print(&doc, &empty, &WIDE), "a\n\nb");
        let expanded = doc.group([text("b"), expand_parent(), text("c")]);
        let expand = [doc.group([text("a"), soft_line_break_or_space(), expanded])];
        assert_eq!(print(&doc, &expand, &WIDE), "a\nbc");
        let multiline = [doc.group([text("a\nb"), soft_line_break_or_space(), text("c")])];
        assert_eq!(print(&doc, &multiline, &WIDE), "a\nb\nc");
        let indented = doc.indent([
            soft_line_break_or_space(),
            text("b"),
            hard_line_break(),
            text("c"),
        ]);
        let in_indent = [doc.group([text("a"), indented])];
        assert_eq!(print(&doc, &in_indent, &WIDE), "a\n  b\n  c");
    }

    #[test]
    fn forced_break_in_line_suffix_breaks_enclosing_group() {
        let mut doc = Document::default();
        let in_group = doc.group([hard_line_break()]);
        let in_suffix = doc.line_suffix([hard_line_break()]);
        let forced_breaks = [
            expand_parent(),
            hard_line_break(),
            text("\n"),
            exact_text("\n"),
            doc.indent([in_group]),
            in_suffix,
        ];
        for forced_break in forced_breaks {
            let suffix = doc.line_suffix([forced_break]);
            let elements = [doc.group([text("a"), suffix, soft_line_break_or_space(), text("b")])];
            assert_eq!(print(&doc, &elements, &WIDE), "a\nb", "{forced_break:?}");
        }
    }

    #[test]
    fn boundary_in_line_suffix_never_ends_measurement() {
        let mut doc = Document::default();
        let suffix = doc.line_suffix([line_suffix_boundary(), hard_line_break()]);
        let elements = [doc.group([text("a"), suffix, soft_line_break_or_space(), text("b")])];
        assert_eq!(print(&doc, &elements, &WIDE), "a\nb");
    }

    #[test]
    fn group_ignores_forced_break_in_following_line_suffix() {
        let mut doc = Document::default();
        let elements = [
            doc.group([text("a"), soft_line_break_or_space(), text("b")]),
            doc.line_suffix([hard_line_break(), text("# c")]),
        ];
        assert_eq!(print(&doc, &elements, &WIDE), "a b\n# c");
    }

    #[test]
    fn groups_inside_broken_group_fit_independently() {
        let mut doc = Document::default();
        let first = doc.group([text("a"), soft_line_break_or_space(), text("b")]);
        let second = doc.group([text("c"), soft_line_break_or_space(), text("d")]);
        let elements = [doc.group([
            text("call"),
            soft_line_break_or_space(),
            first,
            soft_line_break_or_space(),
            second,
        ])];
        assert_eq!(print(&doc, &elements, &narrow(6)), "call\na b\nc d");
    }

    #[test]
    fn rest_of_line_counts_toward_group_fit() {
        let mut doc = Document::default();
        let group = doc.group([text("a"), soft_line_break_or_space(), text("b")]);
        let then_semicolon = [group, text(";")];
        assert_eq!(print(&doc, &then_semicolon, &narrow(4)), "a b;");
        let then_semicolons = [group, text(";;")];
        assert_eq!(print(&doc, &then_semicolons, &narrow(4)), "a\nb;;");
        let then_line = [group, hard_line_break(), text("long line")];
        assert_eq!(print(&doc, &then_line, &narrow(4)), "a b\nlong line");
        let then_text = [group, text("x\r\ny")];
        assert_eq!(print(&doc, &then_text, &narrow(4)), "a bx\ny");
        let then_suffix = [group, doc.line_suffix([text("# a comment")])];
        assert_eq!(print(&doc, &then_suffix, &narrow(4)), "a b# a comment");
    }

    #[test]
    fn indent_takes_columns() {
        let mut doc = Document::default();
        let group = doc.group([text("aa"), soft_line_break_or_space(), text("bb")]);
        let elements = [text("a"), doc.indent([hard_line_break(), group])];
        assert_eq!(print(&doc, &elements, &narrow(7)), "a\n  aa bb");
        assert_eq!(print(&doc, &elements, &narrow(6)), "a\n  aa\n  bb");
        let narrow_tabs = PrinterOptions {
            indent_style: IndentStyle::Tab,
            ..narrow(6)
        };
        assert_eq!(print(&doc, &elements, &narrow_tabs), "a\n\taa\n\tbb");
    }

    #[test]
    fn line_breaks_in_text_become_line_ending() {
        let elements = [text("a\r\nb\nc\rd"), hard_line_break(), text("e")];
        assert_eq!(print_leaves(&elements, &WIDE), "a\nb\nc\rd\ne");
        assert_eq!(print_leaves(&elements, &CRLF), "a\r\nb\r\nc\rd\r\ne");
    }

    #[test]
    fn exact_text_keeps_its_line_breaks() {
        let elements = [exact_text("a\r\nb\nc\r\r\nd"), hard_line_break(), text("e")];
        assert_eq!(print_leaves(&elements, &WIDE), "a\r\nb\nc\r\r\nd\ne");
        assert_eq!(print_leaves(&elements, &CRLF), "a\r\nb\nc\r\r\nd\r\ne");
    }

    #[test]
    fn cr_before_line_break_in_exact_text_takes_no_width() {
        let mut doc = Document::default();
        let group = doc.group([text("a"), soft_line_break_or_space(), text("b")]);
        let elements = [group, exact_text("x\r\ny")];
        assert_eq!(print(&doc, &elements, &narrow(4)), "a bx\r\ny");
        let elements = [group, exact_text("x\r\r\ny")];
        assert_eq!(print(&doc, &elements, &narrow(4)), "a\nbx\r\r\ny");
    }

    #[test]
    fn exact_text_over_lines_breaks_enclosing_group() {
        let mut doc = Document::default();
        let elements = [doc.group([text("a"), soft_line_break_or_space(), exact_text("b\nc")])];
        assert_eq!(print(&doc, &elements, &WIDE), "a\nb\nc");
    }

    #[test]
    fn exact_text_resets_column_after_its_last_line_break() {
        let mut doc = Document::default();
        let elements = [
            exact_text("a\r\nbb"),
            doc.group([text("c"), soft_line_break_or_space(), text("d")]),
        ];
        assert_eq!(print(&doc, &elements, &narrow(5)), "a\r\nbbc d");
        assert_eq!(print(&doc, &elements, &narrow(4)), "a\r\nbbc\nd");
    }

    #[test]
    fn line_break_ending_text_collapses_with_hard_line_break() {
        let elements = [text("a\n"), hard_line_break(), text("b")];
        assert_eq!(print_leaves(&elements, &WIDE), "a\nb");
        assert_eq!(print_leaves(&elements, &CRLF), "a\r\nb");
    }

    #[test]
    fn only_first_line_of_text_is_indented() {
        let mut doc = Document::default();
        let elements = [text("a"), doc.indent([hard_line_break(), text("b\nc")])];
        assert_eq!(print(&doc, &elements, &WIDE), "a\n  b\nc");
    }

    #[test]
    fn empty_first_line_of_text_carries_no_indent() {
        let mut doc = Document::default();
        for multiline in ["\nb", "\r\nb"] {
            let elements = [text("a"), doc.indent([hard_line_break(), text(multiline)])];
            assert_eq!(print(&doc, &elements, &WIDE), "a\n\nb", "{multiline:?}");
            assert_eq!(print(&doc, &elements, &TABS), "a\n\nb", "{multiline:?}");
        }
        for multiline in ["\nb", "\r\nb"] {
            let elements = [
                text("a"),
                doc.indent([hard_line_break(), exact_text(multiline)]),
            ];
            assert_eq!(
                print(&doc, &elements, &WIDE),
                format!("a\n{multiline}"),
                "{multiline:?}"
            );
        }
    }

    #[test]
    fn empty_text_leaves_indent_to_next_text() {
        let mut doc = Document::default();
        let elements = [
            text("a"),
            doc.indent([hard_line_break(), text(""), text("b")]),
        ];
        assert_eq!(print(&doc, &elements, &WIDE), "a\n  b");
    }

    #[test]
    fn multiline_text_resets_column() {
        let mut doc = Document::default();
        let elements = [
            text("a\nbb"),
            doc.group([text("c"), soft_line_break_or_space(), text("d")]),
        ];
        assert_eq!(print(&doc, &elements, &narrow(5)), "a\nbbc d");
    }

    #[test]
    fn line_suffix_keeps_written_indent_and_mode() {
        let mut doc = Document::default();
        let suffix = doc.line_suffix([hard_line_break(), text("# c")]);
        let elements = [
            text("a"),
            doc.indent([suffix]),
            hard_line_break(),
            text("b"),
        ];
        assert_eq!(print(&doc, &elements, &WIDE), "a\n  # c\nb");
        let suffix = doc.line_suffix([space(), text("# c"), soft_line_break(), text("!")]);
        let elements = [
            doc.group([text("a"), suffix, space(), text("b")]),
            hard_line_break(),
            text("d"),
        ];
        assert_eq!(print(&doc, &elements, &WIDE), "a b # c!\nd");
    }

    #[test]
    fn boundary_in_flat_group_keeps_line_suffix_group_flat() {
        let mut doc = Document::default();
        let group = doc.group([text(" #"), soft_line_break_or_space(), text("c")]);
        let elements = [
            text("a"),
            doc.line_suffix([group]),
            doc.group([line_suffix_boundary(), text("b")]),
        ];
        assert_eq!(print(&doc, &elements, &WIDE), "a # c\nb");
    }

    #[test]
    fn line_suffix_waits_for_next_line_break() {
        let mut doc = Document::default();
        let elements = [
            text("a"),
            comment(&mut doc),
            space(),
            text("b"),
            hard_line_break(),
            text("c"),
        ];
        assert_eq!(print(&doc, &elements, &WIDE), "a b # c\nc");
        let elements = [text("a"), comment(&mut doc), empty_line(), text("b")];
        assert_eq!(print(&doc, &elements, &WIDE), "a # c\n\nb");
        let suffix = comment(&mut doc);
        let elements = [doc.group([text("a"), suffix, soft_line_break_or_space(), text("b")])];
        assert_eq!(print(&doc, &elements, &WIDE), "a b # c");
        assert_eq!(print(&doc, &elements, &narrow(2)), "a # c\nb");
    }

    #[test]
    fn line_suffixes_keep_their_order() {
        let mut doc = Document::default();
        let elements = [
            text("a"),
            doc.line_suffix([text(" x")]),
            doc.line_suffix([text(" y")]),
            hard_line_break(),
            text("b"),
        ];
        assert_eq!(print(&doc, &elements, &WIDE), "a x y\nb");
    }

    #[test]
    fn line_suffix_at_end_still_prints() {
        let mut doc = Document::default();
        let elements = [text("a"), comment(&mut doc)];
        assert_eq!(print(&doc, &elements, &WIDE), "a # c");
    }

    #[test]
    fn boundary_breaks_line_only_after_line_suffix() {
        let mut doc = Document::default();
        let elements = [
            text("a"),
            comment(&mut doc),
            line_suffix_boundary(),
            text("b"),
        ];
        assert_eq!(print(&doc, &elements, &WIDE), "a # c\nb");
        let elements = [text("a"), line_suffix_boundary(), text("b")];
        assert_eq!(print_leaves(&elements, &WIDE), "ab");
    }

    #[test]
    fn line_suffix_takes_no_width() {
        let mut doc = Document::default();
        let suffix = doc.line_suffix([text(" # a long comment")]);
        let elements = [doc.group([text("aa"), suffix, soft_line_break_or_space(), text("bb")])];
        assert_eq!(print(&doc, &elements, &narrow(5)), "aa bb # a long comment");
    }

    #[test]
    fn boundary_after_line_suffix_ends_measurement() {
        let mut doc = Document::default();
        let elements = [
            text("aaa"),
            comment(&mut doc),
            doc.group([
                line_suffix_boundary(),
                text("bbb"),
                soft_line_break_or_space(),
                text("ccc"),
            ]),
        ];
        assert_eq!(print(&doc, &elements, &narrow(7)), "aaa # c\nbbb ccc");
    }

    #[test]
    fn alignment_adds_its_columns_to_indent() {
        let mut doc = Document::default();
        let indented = doc.indent([hard_line_break(), text("c")]);
        let aligned = doc.align(1, [hard_line_break(), text("d")]);
        let elements = [
            text("a"),
            doc.align(2, [hard_line_break(), text("b"), indented]),
            doc.indent([aligned]),
        ];
        assert_eq!(print(&doc, &elements, &WIDE), "a\n  b\n    c\n   d");
    }

    #[test]
    fn alignment_before_indent_level_becomes_tab() {
        let mut doc = Document::default();
        let aligned = doc.align(1, [hard_line_break(), text("d")]);
        let indented = doc.indent([hard_line_break(), text("c"), aligned]);
        let elements = [
            text("a"),
            doc.align(2, [hard_line_break(), text("b"), indented]),
        ];
        assert_eq!(print(&doc, &elements, &TABS), "a\n  b\n\t\tc\n\t\t d");
    }

    #[test]
    fn alignment_on_group_follows_whether_it_broke() {
        let mut doc = Document::default();
        let elements = [
            doc.group_with_id(
                GroupId(0),
                [text("aaa"), soft_line_break_or_space(), text("|")],
            ),
            doc.align_if_group_breaks(
                GroupId(0),
                2,
                [space(), text("b"), hard_line_break(), text("c")],
            ),
        ];
        assert_eq!(print(&doc, &elements, &WIDE), "aaa | b\nc");
        assert_eq!(print(&doc, &elements, &narrow(4)), "aaa\n| b\n  c");
    }

    #[test]
    fn indent_on_group_follows_whether_it_broke() {
        let mut doc = Document::default();
        let indented =
            doc.indent_if_group_breaks(GroupId(0), [soft_line_break_or_space(), text("| b")]);
        let elements = [
            doc.group_with_id(GroupId(0), [text("aaa"), indented]),
            doc.indent_if_group_breaks(GroupId(0), [hard_line_break(), text("c")]),
        ];
        assert_eq!(print(&doc, &elements, &WIDE), "aaa | b\nc");
        assert_eq!(print(&doc, &elements, &narrow(4)), "aaa\n  | b\n  c");
    }

    #[test]
    fn indent_on_group_needs_it_already_broken() {
        let mut doc = Document::default();
        let inner = doc.group_with_id(GroupId(3), [text("x"), soft_line_break(), text("y")]);
        let in_flat_parent = [
            doc.group([inner]),
            doc.indent_if_group_breaks(GroupId(3), [hard_line_break(), text("b")]),
        ];
        assert_eq!(print(&doc, &in_flat_parent, &WIDE), "xy\nb");
        let unprinted = [
            doc.indent_if_group_breaks(GroupId(1), [text("a"), hard_line_break(), text("b")]),
            doc.group_with_id(GroupId(1), [text("c"), hard_line_break(), text("d")]),
        ];
        assert_eq!(print(&doc, &unprinted, &WIDE), "a\nbc\nd");
    }

    #[test]
    fn alignment_takes_columns() {
        let mut doc = Document::default();
        let group = doc.group([text("aa"), soft_line_break_or_space(), text("bb")]);
        let elements = [text("a"), doc.align(3, [hard_line_break(), group])];
        assert_eq!(print(&doc, &elements, &narrow(8)), "a\n   aa bb");
        assert_eq!(print(&doc, &elements, &narrow(7)), "a\n   aa\n   bb");
    }

    #[test]
    fn alignment_takes_columns_after_tabs_too() {
        let mut doc = Document::default();
        let group = doc.group([text("bb"), soft_line_break_or_space(), text("cc")]);
        let aligned = doc.align(2, [hard_line_break(), group]);
        let elements = [text("a"), doc.indent([aligned])];
        let four_column_tabs = |line_width| PrinterOptions {
            line_width,
            indent_width: 4,
            ..TABS
        };
        assert_eq!(
            print(&doc, &elements, &four_column_tabs(11)),
            "a\n\t  bb cc"
        );
        assert_eq!(
            print(&doc, &elements, &four_column_tabs(10)),
            "a\n\t  bb\n\t  cc"
        );
    }

    #[test]
    fn zero_columns_are_no_alignment() {
        let mut doc = Document::default();
        let indented = doc.indent([hard_line_break(), text("b")]);
        let elements = [text("a"), doc.align(0, [indented])];
        assert_eq!(print(&doc, &elements, &TABS), "a\n\tb");
    }

    #[test]
    fn alignment_on_group_needs_it_already_broken() {
        let mut doc = Document::default();
        let inner = doc.group_with_id(GroupId(0), [text("a")]);
        let aligned = doc.align_if_group_breaks(GroupId(0), 2, [hard_line_break(), text("b")]);
        let flat = [doc.group([inner, aligned])];
        assert_eq!(print(&doc, &flat, &WIDE), "a\nb");
        let inner = doc.group_with_id(GroupId(3), [text("x"), soft_line_break(), text("y")]);
        let in_flat_parent = [
            doc.group([inner]),
            doc.align_if_group_breaks(GroupId(3), 2, [hard_line_break(), text("b")]),
        ];
        assert_eq!(print(&doc, &in_flat_parent, &WIDE), "xy\nb");
        let unprinted = [
            doc.align_if_group_breaks(GroupId(1), 2, [text("a"), hard_line_break(), text("b")]),
            doc.group_with_id(GroupId(1), [text("c"), hard_line_break(), text("d")]),
        ];
        assert_eq!(print(&doc, &unprinted, &WIDE), "a\nbc\nd");
        let aligned = doc.align_if_group_breaks(GroupId(2), 2, [hard_line_break(), text("z")]);
        let inside_own = [doc.group_with_id(
            GroupId(2),
            [text("xx"), soft_line_break_or_space(), text("yy"), aligned],
        )];
        assert_eq!(print(&doc, &inside_own, &WIDE), "xx\nyy\n  z");
    }

    #[test]
    fn alignment_spares_empty_lines_not_line_suffixes() {
        let mut doc = Document::default();
        let suffix = doc.line_suffix([hard_line_break(), text("# c")]);
        let elements = [
            text("a"),
            doc.align(2, [empty_line(), text("b"), suffix]),
            hard_line_break(),
            text("d"),
        ];
        assert_eq!(print(&doc, &elements, &WIDE), "a\n\n  b\n  # c\nd");
    }
}
