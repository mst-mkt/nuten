//! Formatting rules for Nushell.

pub mod invariants;
mod rules;

use nuten_doc::{Document, PrinterOptions};
pub use nuten_doc::{IndentStyle, LineEnding};
use nuten_parser::Parsed;
pub use nuten_parser::{ParseError, ParseErrorKind};
use nuten_syntax::SyntaxNode;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Config {
    /// Columns a line may take before it breaks where it can.
    pub line_width: u32,
    pub indent_style: IndentStyle,
    /// Columns per indent level, and per tab when indenting with tabs.
    pub indent_width: u8,
    pub line_ending: LineEnding,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            line_width: 100,
            indent_style: IndentStyle::Space,
            indent_width: 4,
            line_ending: LineEnding::Lf,
        }
    }
}

/// Syntax errors; a file with any is not formatted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FormatError {
    /// At least one, in the order they appear in the source.
    pub errors: Vec<ParseError>,
}

/// Formats a whole file. The output ends with one line break, or is empty
/// when the file is blank.
///
/// # Errors
///
/// When the file has syntax errors.
pub fn format(src: &str, config: &Config) -> Result<String, FormatError> {
    format_parsed(src, &nuten_parser::parse(src), config)
}

/// `parsed` is the tree of `src`.
fn format_parsed(src: &str, parsed: &Parsed, config: &Config) -> Result<String, FormatError> {
    if !parsed.errors().is_empty() {
        return Err(FormatError {
            errors: parsed.errors().to_vec(),
        });
    }
    let root = preprocess(parsed.syntax());
    let document = Document {
        elements: rules::file(src, &root),
    };
    let options = PrinterOptions {
        line_width: config.line_width,
        indent_style: config.indent_style,
        indent_width: config.indent_width,
        line_ending: config.line_ending,
    };
    Ok(nuten_doc::print(&document, &options))
}

/// Where tree rewrites before formatting go. None yet.
/// The rules slice the source by token ranges, so the text must stay.
fn preprocess(root: SyntaxNode) -> SyntaxNode {
    root
}

#[cfg(test)]
mod tests {
    use super::{Config, ParseErrorKind, format};

    #[test]
    fn a_file_with_errors_is_rejected() {
        let error = format("ls |", &Config::default()).unwrap_err();
        assert_eq!(error.errors.len(), 1);
        assert_eq!(error.errors[0].kind, ParseErrorKind::PipeAtEof);
        assert_eq!(&"ls |"[error.errors[0].range], "|");
    }

    #[test]
    fn only_a_blank_file_comes_out_empty() {
        for src in ["", "\n", "  \n\n", "\r\n"] {
            assert_eq!(format(src, &Config::default()).unwrap(), "", "{src:?}");
        }
        assert_eq!(format("# c", &Config::default()).unwrap(), "# c\n");
    }

    #[test]
    fn the_output_ends_with_one_line_break() {
        for src in ["ls", "ls\n", "ls \n\n", "ls\r\n", "\n\nls\n\n\n"] {
            assert_eq!(format(src, &Config::default()).unwrap(), "ls\n", "{src:?}");
        }
    }

    #[test]
    fn a_blank_line_keeps_a_line_end_pipe_from_joining_the_next_statement() {
        let src = "ls |\n\nlength\n";
        assert_eq!(format(src, &Config::default()).unwrap(), src);
    }
}
