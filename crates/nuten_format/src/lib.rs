pub mod invariants;
mod rules;

use std::ops::RangeInclusive;

use nuten_doc::PrinterOptions;
use nuten_parser::Parsed;
pub use nuten_parser::{ParseError, ParseErrorKind};
use nuten_syntax::{SyntaxElement, SyntaxNode};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Config {
    pub line_width: u32,
    pub indent_style: IndentStyle,
    pub indent_width: u8,
    pub line_ending: LineEnding,
    pub commas: Commas,
}

impl Config {
    pub const INDENT_WIDTHS: RangeInclusive<u8> = 1..=u8::MAX;
    pub const LINE_WIDTHS: RangeInclusive<u32> = 1..=u32::MAX;
}

impl Default for Config {
    fn default() -> Self {
        Self {
            line_width: 100,
            indent_style: IndentStyle::Space,
            indent_width: 4,
            line_ending: LineEnding::Auto,
            commas: Commas::Inline,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Commas {
    None,
    Inline,
    Always,
    Trailing,
}

impl Commas {
    pub const ALL: [Commas; 4] = [
        Commas::None,
        Commas::Inline,
        Commas::Always,
        Commas::Trailing,
    ];

    pub const fn name(self) -> &'static str {
        match self {
            Commas::None => "none",
            Commas::Inline => "inline",
            Commas::Always => "always",
            Commas::Trailing => "trailing",
        }
    }

    pub fn of_name(name: &str) -> Option<Commas> {
        Commas::ALL.into_iter().find(|commas| commas.name() == name)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IndentStyle {
    Space,
    Tab,
}

impl IndentStyle {
    pub const ALL: [IndentStyle; 2] = [IndentStyle::Space, IndentStyle::Tab];

    pub const fn name(self) -> &'static str {
        match self {
            IndentStyle::Space => "space",
            IndentStyle::Tab => "tab",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineEnding {
    Lf,
    CrLf,
    Auto,
}

impl LineEnding {
    pub const ALL: [LineEnding; 3] = [LineEnding::Lf, LineEnding::CrLf, LineEnding::Auto];

    pub const fn name(self) -> &'static str {
        match self {
            LineEnding::Lf => "lf",
            LineEnding::CrLf => "crlf",
            LineEnding::Auto => "auto",
        }
    }
}

fn printer_options(config: &Config, root: &SyntaxNode) -> PrinterOptions {
    let indent_style = match config.indent_style {
        IndentStyle::Space => nuten_doc::IndentStyle::Space,
        IndentStyle::Tab => nuten_doc::IndentStyle::Tab,
    };
    let line_ending = match config.line_ending {
        LineEnding::Lf => nuten_doc::LineEnding::Lf,
        LineEnding::CrLf => nuten_doc::LineEnding::CrLf,
        LineEnding::Auto => first_line_ending(root),
    };
    PrinterOptions {
        line_width: config.line_width,
        indent_style,
        indent_width: config.indent_width,
        line_ending,
    }
}

/// Only a line break that is layout tells, since the others stay as written.
/// The first such break in the output is one the printer makes.
/// So formatting the output again takes the same line ending.
fn first_line_ending(root: &SyntaxNode) -> nuten_doc::LineEnding {
    let crlf = root
        .descendants_with_tokens()
        .filter_map(SyntaxElement::into_token)
        .find(rules::is_layout_line_break)
        .is_some_and(|newline| newline.text() == "\r\n");
    if crlf {
        nuten_doc::LineEnding::CrLf
    } else {
        nuten_doc::LineEnding::Lf
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FormatError {
    /// At least one.
    pub errors: Vec<ParseError>,
}

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
    let root = parsed.syntax();
    let document = rules::file(src, &root, config.commas);
    Ok(nuten_doc::print(&document, &printer_options(config, &root)))
}

#[cfg(test)]
mod tests {
    use super::{Commas, Config, IndentStyle, LineEnding, ParseErrorKind, format, invariants};

    #[test]
    fn commas_all_holds_each_commas_once_in_fixed_order() {
        let position = |commas| match commas {
            Commas::None => 0,
            Commas::Inline => 1,
            Commas::Always => 2,
            Commas::Trailing => 3,
        };
        assert_eq!(Commas::ALL.map(position), [0, 1, 2, 3]);
    }

    #[test]
    fn commas_of_name_gives_back_each_commas_by_its_name() {
        for commas in Commas::ALL {
            assert_eq!(Commas::of_name(commas.name()), Some(commas));
        }
    }

    #[test]
    fn commas_of_name_takes_names_in_lowercase_only() {
        assert_eq!(Commas::of_name("Inline"), None);
    }

    #[test]
    fn indent_style_all_holds_each_indent_style_once_in_fixed_order() {
        let position = |indent_style| match indent_style {
            IndentStyle::Space => 0,
            IndentStyle::Tab => 1,
        };
        assert_eq!(IndentStyle::ALL.map(position), [0, 1]);
    }

    #[test]
    fn line_ending_all_holds_each_line_ending_once_in_fixed_order() {
        let position = |line_ending| match line_ending {
            LineEnding::Lf => 0,
            LineEnding::CrLf => 1,
            LineEnding::Auto => 2,
        };
        assert_eq!(LineEnding::ALL.map(position), [0, 1, 2]);
    }

    #[test]
    fn file_with_syntax_errors_is_rejected() {
        let error = format("ls |", &Config::default()).unwrap_err();
        assert_eq!(error.errors.len(), 1);
        assert_eq!(error.errors[0].kind, ParseErrorKind::PipeAtEof);
        assert_eq!(&"ls |"[error.errors[0].range], "|");
    }

    #[test]
    fn only_blank_files_come_out_empty() {
        for src in ["", "\n", "  \n\n", "\r\n"] {
            assert_eq!(format(src, &Config::default()).unwrap(), "", "{src:?}");
        }
        assert_eq!(format("# c", &Config::default()).unwrap(), "# c\n");
    }

    #[test]
    fn output_ends_with_one_line_break() {
        for src in ["ls", "ls\n", "ls \n\n", "\n\nls\n\n\n"] {
            assert_eq!(format(src, &Config::default()).unwrap(), "ls\n", "{src:?}");
        }
        for src in ["ls\r\n", "ls\r\n\r\n", "\r\nls\r\n"] {
            assert_eq!(
                format(src, &Config::default()).unwrap(),
                "ls\r\n",
                "{src:?}"
            );
        }
    }

    #[test]
    fn crlf_keeps_carriage_returns_before_line_break() {
        let crlf = Config {
            line_ending: LineEnding::CrLf,
            ..Config::default()
        };
        for (src, expected) in [
            ("ls # d\r\r\r\n", "ls # d\r\r\r\n"),
            ("echo 'a\r\r\nb'\r\n", "echo 'a\r\r\nb'\r\n"),
            ("[a # c\r]\r\n", "[\r\n    a # c\r\r\n]\r\n"),
        ] {
            assert_eq!(format(src, &crlf).unwrap(), expected, "{src:?}");
            let checked = invariants::check(src, &crlf);
            assert_eq!(checked.violations, [], "{src:?}");
        }
    }

    #[test]
    fn crlf_leaves_line_breaks_inside_tokens_as_written() {
        let crlf = Config {
            line_ending: LineEnding::CrLf,
            ..Config::default()
        };
        for (src, expected) in [
            ("echo 'a\nb'\n", "echo 'a\nb'\r\n"),
            ("echo foo[\n1]bar\n", "echo foo[\n1]bar\r\n"),
            (
                "# nuten-ignore\n[1   'a\nb'\n  2]\n",
                "# nuten-ignore\r\n[1   'a\nb'\r\n  2]\r\n",
            ),
        ] {
            assert_eq!(format(src, &crlf).unwrap(), expected, "{src:?}");
            let checked = invariants::check(src, &crlf);
            assert_eq!(checked.violations, [], "{src:?}");
        }
    }

    #[test]
    fn crlf_leaves_line_breaks_inside_example_block_as_written() {
        let crlf = Config {
            line_ending: LineEnding::CrLf,
            ..Config::default()
        };
        let src = "@example \"d\" {\n  [1\n  2]\n} --result [1 2]\ndef f [] {}\n";
        assert_eq!(
            format(src, &crlf).unwrap(),
            "@example \"d\" {\n  [1\n  2]\n} --result [1 2]\r\ndef f [] {}\r\n"
        );
        assert_eq!(invariants::check(src, &crlf).violations, []);
    }

    #[test]
    fn nesting_past_depth_limit_keeps_invariants() {
        for src in [
            format!("mut a = 1\n{}1\n", "$a  =  ".repeat(1_000)),
            format!("{}1\n", "let a  =  ".repeat(1_000)),
            format!("{}1\n", "1  +  if true {  1 } else  ".repeat(1_000)),
            format!(
                "{}true{}\n",
                "if 1  +  ".repeat(1_000),
                " {  1 }".repeat(1_000)
            ),
        ] {
            let checked = invariants::check(&src, &Config::default());
            assert!(checked.syntax_errors.is_empty(), "{src:?}");
            assert_eq!(checked.violations, [], "{src:?}");
        }
    }

    #[test]
    fn line_end_pipe_keeps_blank_line() {
        let src = "ls |\n\nlength\n";
        assert_eq!(format(src, &Config::default()).unwrap(), src);
    }

    #[test]
    fn two_blank_lines_separate_line_end_and_leading_pipes() {
        // With one, the `|` below would join the statement above.
        for (src, expected) in [
            ("ls |\n\n\n| length\n", "ls |\n\n\n| length\n"),
            ("ls |\n\n\n\n| length\n", "ls |\n\n\n| length\n"),
            ("ls |\n\n\n# c\n| length\n", "ls |\n\n\n# c\n| length\n"),
            (
                "ls |\n# a\n# b\n\n\n| length\n",
                "ls |\n# a\n# b\n\n\n| length\n",
            ),
            (
                "ls |\n# a\n\n# b\n\n\n| length\n",
                "ls |\n# a\n\n# b\n\n| length\n",
            ),
            (
                "foo\n\n\nls |\n\n\n| length\n",
                "foo\n\nls |\n\n\n| length\n",
            ),
            (
                "do {\n    ls |\n    # a\n    # b\n\n\n    | length\n}\n",
                "do {\n    ls |\n    # a\n    # b\n\n\n    | length\n}\n",
            ),
            (
                "do {\n    ls |\n    # a\n\n    # b\n\n\n    | length\n}\n",
                "do {\n    ls |\n    # a\n\n    # b\n\n    | length\n}\n",
            ),
        ] {
            assert_eq!(
                format(src, &Config::default()).unwrap(),
                expected,
                "{src:?}"
            );
        }
    }
}
