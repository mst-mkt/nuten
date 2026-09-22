//! Grammar rules of Nushell.

mod assignment;
mod binary;
mod block;
mod builtin;
mod cell_path;
mod command;
mod control;
mod def;
mod if_;
mod keyword;
mod list;
mod match_;
mod number;
mod pipeline;
mod range;
mod record;
mod signature;
mod spread;
mod table;
mod variable;

use std::rc::Rc;

use nuten_syntax::SyntaxKind;

use self::{
    assignment::Assigned,
    binary::{binary, binary_ahead},
    block::block,
    cell_path::{cell_path, cell_path_ahead},
    list::{is_list, list_items},
    pipeline::pipeline,
    range::{range, range_ahead},
    record::{RecordSign, record, record_sign},
    spread::{Target, is_spread, spread},
    table::{is_table, table_items},
    variable::variable_error,
};
use crate::{
    ParseErrorKind,
    lexer::Mode,
    parser::{Bracketed, Brackets, Parser},
};

pub(crate) fn file(p: &mut Parser<'_>) {
    while let Some(kind) = p.peek() {
        match kind {
            SyntaxKind::Semicolon if p.last_kind() == Some(SyntaxKind::Pipe) => {
                p.bump_error(ParseErrorKind::SemicolonAfterPipe);
            },
            kind if ends_statement(kind) => p.bump(),
            _ => pipeline(p, Assigned::No),
        }
    }
}

/// The names `src` aliases, sorted so that the parser can bisect them.
/// Reads the text, not the tree, which the parser has yet to build.
/// Taking in a name out of scope only leaves a call read as it was written.
pub(crate) fn alias_names(src: &str) -> Rc<[&str]> {
    let mut names: Vec<_> = src.split(breaks_statement).filter_map(alias_name).collect();
    names.sort_unstable();
    names.dedup();
    names.into()
}

/// What nu may start a statement after, so that an `alias` first in one is a head.
fn breaks_statement(char: char) -> bool {
    matches!(char, '\n' | '\r' | ';' | '|' | '{' | '}' | '(' | ')')
}

const NAME_QUOTES: [char; 3] = ['"', '\'', '`'];

fn alias_name(statement: &str) -> Option<&str> {
    let mut words = statement.split_ascii_whitespace();
    let first = words.next()?;
    let head = if first == "export" {
        words.next()?
    } else {
        first
    };
    if head != "alias" {
        return None;
    }
    Some(words.next()?.trim_matches(NAME_QUOTES))
}

/// What nu takes at a position, as far as it tells how nuten reads a word there.
/// Nu's `SyntaxShape` decides more; these are the ones that tell here.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Shape {
    /// Any value: `[ ]` makes a list or a table, `{ }` a record or a block.
    Any,
    /// An argument of a command nu resolves to one of its own.
    /// A `[ ]` stays a word, since the signature may take it as a string.
    /// A `{ }` is never a string, unlike for an external command.
    /// A spread first stays a word: only the signature tells a record from a closure there.
    /// An argument like `...$x` stays a word as well.
    /// Nu reads no spread where a flag takes it as its value, which only the signature tells.
    KnownArgument,
    /// An argument of a command that takes a row condition.
    /// Nu reads a `{ }` that shows a `RecordSign` as a block alone, and as a value among others.
    /// So it stays a word, which reads the same either way.
    RowCondition,
    /// An argument of a head nuten cannot look up: an alias, one the file defines, or none.
    /// Every bracketed word but `( )` stays one, since what nu reads it as is unknowable here.
    /// So does an argument like `...$x`, as after a command nu resolves to one of its own.
    UnknownArgument,
    /// What a `let`, a `mut`, or a `const` takes before its operator: the name and any type.
    /// Nu reads those as written, never as a value, so not even `( )` is one.
    VariableName,
    /// An argument of an external command: a `[ ]` is a list, and a `{ }` a string.
    /// A `...` before a `$`, a `( )`, or a `[ ]` makes a spread, as in a list.
    ExternalArgument,
    /// A block or a closure: the body of a keyword syntax, what `else` takes, what an arm does.
    /// Nu reads a `{ }` there as a record only for a `:` second.
    /// One with a spread first is a block, unlike where nu takes any value.
    Block,
    /// The body of a `def`, which nu reads as a block without looking at what the `{ }` holds.
    DefBody,
}

/// What nu reads a `{ }` as.
enum Braced {
    /// With the `RecordSign` it shows.
    Record(RecordSign),
    Block,
    Word,
}

impl Shape {
    /// Whether nu reads a value here at all.
    /// It reads a `$` or a `( )` as one before it looks at the shape.
    fn takes_value(self) -> bool {
        self != Shape::VariableName
    }

    /// Whether nu reads a `[ ]` as a list where it takes this shape.
    fn reads_list(self) -> bool {
        matches!(self, Shape::Any | Shape::Block | Shape::ExternalArgument)
    }

    /// Whether nu reads a `[ ]` as a table where it takes this shape.
    fn reads_table(self) -> bool {
        self.takes_any_value()
    }

    /// Whether nu reads a `[ ]` here as it does where it takes any value.
    fn takes_any_value(self) -> bool {
        matches!(self, Shape::Any | Shape::Block)
    }

    /// What nu reads a `{ }` as here, given the `RecordSign` its first tokens show.
    fn braced(self, sign: Option<RecordSign>) -> Braced {
        match (self, sign) {
            (Shape::Any, Some(sign))
            | (Shape::KnownArgument | Shape::Block, Some(sign @ RecordSign::ColonSecond)) => {
                Braced::Record(sign)
            },
            (Shape::KnownArgument, Some(RecordSign::SpreadFirst))
            | (Shape::RowCondition, Some(_))
            | (Shape::UnknownArgument | Shape::VariableName | Shape::ExternalArgument, _) => {
                Braced::Word
            },
            (Shape::Any | Shape::KnownArgument | Shape::RowCondition, None)
            | (Shape::Block, None | Some(RecordSign::SpreadFirst))
            | (Shape::DefBody, _) => Braced::Block,
        }
    }
}

/// An item that is no shell operator, which nu rejects, as `value` reads it.
/// A spread where nu reads one as an argument.
fn item(p: &mut Parser<'_>, shape: Shape) {
    let text = p.peek_text();
    if let Some(kind) = text.and_then(shell_operator) {
        p.bump_error(kind);
    } else if shape == Shape::ExternalArgument
        && text.is_some_and(|text| is_spread(text, Target::List))
    {
        spread(p, Target::List);
    } else {
        value(p, shape);
    }
}

pub(crate) const SQUARE: Brackets = Brackets {
    open: b'[',
    close: b']',
    open_kind: SyntaxKind::LBracket,
    close_kind: SyntaxKind::RBracket,
    always_nested: true,
};

const PAREN: Brackets = Brackets {
    open: b'(',
    close: b')',
    open_kind: SyntaxKind::LParen,
    close_kind: SyntaxKind::RParen,
    always_nested: true,
};

const CURLY: Brackets = Brackets {
    open: b'{',
    close: b'}',
    open_kind: SyntaxKind::LBrace,
    close_kind: SyntaxKind::RBrace,
    always_nested: true,
};

/// The next token as the value nu reads where it takes `shape`.
/// A `Binary`, `Range`, `CellPath`, or `Subexpression` where nu reads one.
/// A `List`, `Table`, `Record`, or `Block` too.
/// Otherwise the token as it is, in an `Error` where nu reports one at a variable.
///
/// The `Def` rule reads the one a signature stands in before `value` sees it.
fn value(p: &mut Parser<'_>, shape: Shape) {
    if let Some(ahead) = binary_ahead(p, shape) {
        binary(p, ahead);
    } else if let Some(ahead) = range_ahead(p, shape) {
        range(p, ahead);
    } else if let Some(ahead) = cell_path_ahead(p, shape) {
        cell_path(p, ahead, shape);
    } else if shape.takes_value()
        && let Some(bracketed) = p.bracketed(&PAREN, Mode::Paren)
    {
        p.bump_bracketed(SyntaxKind::Subexpression, bracketed, file);
    } else if shape.reads_list()
        && let Some(bracketed) = p.bracketed(&SQUARE, Mode::List)
    {
        squared(p, bracketed, shape);
    } else if let Some(bracketed) = p.bracketed(&CURLY, Mode::Brace) {
        braced(p, bracketed, shape);
    } else if let Some(error) = variable_error(p, shape) {
        p.bump_error(error);
    } else {
        p.bump();
    }
}

/// The `[ ]` ahead as nu reads it where it takes `shape`: a `List`, a `Table`, or a word.
/// `bracketed` is what it holds in list mode.
fn squared<'a>(p: &mut Parser<'a>, bracketed: Bracketed<'a>, shape: Shape) {
    if is_list(bracketed.inner()) {
        p.bump_bracketed(SyntaxKind::List, bracketed, list_items);
    } else if shape.reads_table() && is_table(bracketed.inner()) {
        p.bump_bracketed(SyntaxKind::Table, bracketed, table_items);
    } else {
        p.bump();
    }
}

/// The `{ }` ahead as nu reads it where it takes `shape`: a `Record`, a `Block`, or a word.
/// `bracketed` is what it holds in brace mode, which tells a record from the rest.
fn braced<'a>(p: &mut Parser<'a>, bracketed: Bracketed<'a>, shape: Shape) {
    match shape.braced(record_sign(bracketed.inner().remaining_tokens())) {
        Braced::Record(sign) => record(p, bracketed.lexed_in(Mode::Record), sign),
        Braced::Block => block(p, bracketed.lexed_in(Mode::Block)),
        Braced::Word => p.bump(),
    }
}

fn ends_statement(kind: SyntaxKind) -> bool {
    matches!(kind, SyntaxKind::Newline | SyntaxKind::Semicolon)
}

fn ends_element(kind: SyntaxKind) -> bool {
    is_pipe(kind) || ends_statement(kind)
}

fn ends_call(kind: SyntaxKind) -> bool {
    is_file_redirection(kind) || kind == SyntaxKind::Assign || ends_element(kind)
}

/// Whether an item the call still takes stands ahead.
fn at_call_item(p: &Parser<'_>) -> bool {
    p.peek().is_some_and(|kind| !ends_call(kind))
}

/// The texts of the items ahead that a call takes.
fn call_items<'a>(p: &Parser<'a>) -> Vec<&'a str> {
    p.remaining_tokens()
        .filter(|(token, _)| !p.is_skipped(token.kind))
        .take_while(|(token, _)| !ends_call(token.kind))
        .map(|(_, text)| text)
        .collect()
}

fn is_pipe(kind: SyntaxKind) -> bool {
    matches!(
        kind,
        SyntaxKind::Pipe | SyntaxKind::ErrGtPipe | SyntaxKind::OutErrGtPipe
    )
}

fn is_file_redirection(kind: SyntaxKind) -> bool {
    matches!(
        kind,
        SyntaxKind::OutGt
            | SyntaxKind::OutGtGt
            | SyntaxKind::ErrGt
            | SyntaxKind::ErrGtGt
            | SyntaxKind::OutErrGt
            | SyntaxKind::OutErrGtGt
    )
}

/// Nu rejects `||`, `&&`, `2>`, `2>&1`, and `o>|` wherever they stand alone.
fn shell_operator(text: &str) -> Option<ParseErrorKind> {
    let kind = match text {
        "||" => ParseErrorKind::ShellOrOr,
        "&&" => ParseErrorKind::ShellAndAnd,
        "2>" => ParseErrorKind::ShellErrRedirect,
        "2>&1" => ParseErrorKind::ShellOutErrRedirect,
        "o>|" | "out>|" => ParseErrorKind::OutPipe,
        _ => return None,
    };
    Some(kind)
}

#[cfg(test)]
mod tests {
    use nuten_syntax::SyntaxKind;

    use super::alias_names;
    use crate::parse;

    #[test]
    fn brackets_nested_too_deep_stay_a_word() {
        let src = format!("{}{}", "[".repeat(130), "]".repeat(130));
        let tree = parse(&src).syntax();
        let lists = tree
            .descendants()
            .filter(|node| node.kind() == SyntaxKind::List);
        assert_eq!(lists.count(), 128);
        let innermost = tree
            .descendants_with_tokens()
            .filter(|element| element.kind() == SyntaxKind::Word);
        assert_eq!(
            innermost.map(|word| word.to_string()).collect::<Vec<_>>(),
            ["[[]]"]
        );
    }

    #[test]
    fn records_nested_too_deep_stay_a_word() {
        let src = format!("{}1{}", "{a: ".repeat(130), "}".repeat(130));
        let tree = parse(&src).syntax();
        let records = tree
            .descendants()
            .filter(|node| node.kind() == SyntaxKind::Record);
        assert_eq!(records.count(), 128);
        let innermost = tree.descendants_with_tokens().last().unwrap();
        assert_eq!(innermost.to_string(), "}");
        assert!(
            tree.descendants_with_tokens()
                .any(|element| element.kind() == SyntaxKind::Word
                    && element.to_string() == "{a: {a: 1}}")
        );
    }

    #[test]
    fn blocks_nested_too_deep_stay_a_word() {
        let src = format!("{}ls{}", "{".repeat(130), "}".repeat(130));
        let tree = parse(&src).syntax();
        let blocks = tree
            .descendants()
            .filter(|node| node.kind() == SyntaxKind::Block);
        assert_eq!(blocks.count(), 128);
        assert!(
            tree.descendants_with_tokens().any(
                |element| element.kind() == SyntaxKind::Word && element.to_string() == "{{ls}}"
            )
        );
    }

    #[test]
    fn alias_names_come_from_every_statement() {
        assert_eq!(*alias_names("alias a = ^a"), ["a"]);
        assert_eq!(*alias_names("export alias b = ^b"), ["b"]);
        assert_eq!(*alias_names("print hi;alias c = ^c"), ["c"]);
        assert_eq!(*alias_names("print hi\ralias d = ^d"), ["d"]);
        assert_eq!(*alias_names("ls | alias e = ^e"), ["e"]);
        assert_eq!(*alias_names("do { alias f = ^f }"), ["f"]);
        assert_eq!(*alias_names("(alias g = ^g)"), ["g"]);
    }

    #[test]
    fn alias_names_drop_quotes_around_name() {
        assert_eq!(*alias_names("alias \"a\" = ^a"), ["a"]);
        assert_eq!(*alias_names("alias 'b' = ^b"), ["b"]);
        assert_eq!(*alias_names("alias `c` = ^c"), ["c"]);
    }

    #[test]
    fn alias_names_come_sorted_without_repeats() {
        assert_eq!(
            *alias_names("alias b = ^b\nalias a = ^a\nalias b = ^c"),
            ["a", "b"]
        );
    }

    #[test]
    fn alias_names_leave_out_what_heads_no_statement() {
        assert!(alias_names("# alias a = ^a").is_empty());
        assert!(alias_names("print \"alias b = ^b\"").is_empty());
    }

    #[test]
    fn alias_names_leave_out_statement_that_names_nothing() {
        assert!(alias_names("alias").is_empty());
        assert!(alias_names("export").is_empty());
        assert!(alias_names("export alias").is_empty());
    }
}
