mod builder;
mod document;
mod element;
mod flat;
mod printer;

pub use builder::{
    Build, Builder, empty_line, empty_lines, exact_text, expand_parent, hard_line_break,
    line_suffix_boundary, soft_line_break, soft_line_break_or_space, space, text,
};
pub use document::{Children, Document, Sides};
pub use element::{Columns, Element, GroupId, GroupIds, LineMode, TextWidth};
pub use flat::breaks_groups;
use flat::flat;
pub use printer::{IndentStyle, LineEnding, PrinterOptions, print};
