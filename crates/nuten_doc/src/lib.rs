//! Document IR and printer, with no knowledge of Nushell.
//! Groups stay on one line when they fit and break otherwise.

mod builder;
mod element;
mod flat;
mod printer;

pub use builder::{
    align, align_if_group_breaks, empty_line, empty_lines, exact_text, expand_parent, group,
    group_with_id, hard_line_break, indent, indent_if_group_breaks, line_suffix,
    line_suffix_boundary, soft_line_break, soft_line_break_or_space, space, text,
};
pub use element::{Element, GroupId, GroupIds, LineMode, TextWidth};
pub use flat::{breaks_groups, flat};
pub use printer::{IndentStyle, LineEnding, PrinterOptions, print};
