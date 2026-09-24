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
}

/// Spaces and tabs before a line break of the printer's go, even those of a text.
/// The lines inside a text stay as they are.
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
                ) => return columns(first_line(text)) <= remaining,
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
        let empty_first_line = width == TextWidth::Multiline && first_line(text).is_empty();
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

    /// Ends the line, dropping the spaces and tabs before the break.
    /// Prints `breaks` line breaks, less those the output ends with already.
    /// A text's own line breaks count among those.
    /// Prints nothing at the start of the output, which counts as an empty line.
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

fn first_line(text: &str) -> &str {
    without_cr(text.split_once('\n').map_or(text, |(first, _)| first))
}

/// `line` came before a `\n`; the `\r` of a `\r\n` is part of the break.
fn without_cr(line: &str) -> &str {
    line.strip_suffix('\r').unwrap_or(line)
}

#[cfg(test)]
mod tests;
