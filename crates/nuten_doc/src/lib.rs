//! Document IR and printer, with no knowledge of Nushell.
//! Groups stay on one line when they fit and break otherwise.

mod builder;
mod element;

pub use builder::{
    empty_line, expand_parent, group, hard_line_break, indent, line_suffix, line_suffix_boundary,
    soft_line_break, soft_line_break_or_space, space, text,
};
pub use element::{Document, Element, LineMode, TextWidth};
