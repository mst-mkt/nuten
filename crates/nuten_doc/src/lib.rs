//! Document IR and printer, with no knowledge of Nushell.
//! Groups stay on one line when they fit and break otherwise.

mod element;

pub use element::{Document, Element, LineMode, TextWidth};
