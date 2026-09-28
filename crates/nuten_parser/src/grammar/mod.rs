//! Grammar rules of Nushell.

mod alias;
mod assignment;
mod attribute;
mod binary;
mod block;
mod builtin;
mod cell_path;
mod command;
mod control;
mod def;
mod if_;
mod interpolation;
mod keyword;
mod list;
mod match_;
mod module;
mod names;
mod number;
mod parameters;
mod pattern;
mod pipeline;
mod range;
mod record;
mod signature;
mod spread;
mod table;
mod use_;
mod var_type;
mod variable;
mod where_;

use nuten_syntax::SyntaxKind;

pub use self::command::operator_precedence;
use self::{
    assignment::Assigned,
    binary::{binary, binary_ahead},
    block::block,
    cell_path::{cell_path, cell_path_ahead},
    interpolation::interpolation,
    list::{is_list, items_as, list_items},
    parameters::Takes,
    pipeline::pipeline,
    range::{range, range_ahead},
    record::{RecordSign, record, record_sign},
    spread::{Target, is_spread, spread},
    table::{is_table, table_items},
    variable::variable_error,
};
pub(crate) use self::{
    names::{Callee, Names},
    parameters::Param,
};
use crate::{
    ParseErrorKind,
    lexer::Mode,
    parser::{Bracketed, Brackets, Enclosed, Parser},
};

pub(crate) fn file(p: &mut Parser<'_>) {
    p.enter_block();
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

/// What nu takes at a position, as far as it tells how nuten reads a word there.
/// Nu's `SyntaxShape` decides more; these are the ones that tell here.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Shape {
    /// Any value: `[ ]` makes a list or a table, `{ }` a record or a block.
    Any,
    /// An argument of a command nu resolves to one of its own.
    /// A `[ ]` stays a word, since the signature may take it as a string.
    /// A `{ }` is never a string, unlike a `[ ]`.
    /// An empty one stays a word, and so does one with a spread first.
    /// Only the signature tells a record from a closure there.
    /// An argument like `...$x` stays a word as well.
    /// Nu reads no spread where a flag takes it as its value, which only the signature tells.
    KnownArgument,
    /// An argument of a command that takes a row condition.
    /// Nu reads a `{ }` that shows a `RecordSign` as a block alone, and as a value among others.
    /// So it stays a word, which reads the same either way.
    RowCondition,
    /// An argument of a head nuten cannot look up: an alias, or one no list or block names.
    /// So is one an import before it may bring, in place of one a block defines or nu has.
    /// So is one a block around it defines, where a `hide` may take it away.
    /// Every bracketed word but `( )` stays one, since what nu reads it as is unknowable here.
    /// So does an argument like `...$x`, as after a command nu resolves to one of its own.
    UnknownArgument,
    /// What a `let`, a `mut`, or a `const` takes before its operator: the name and any type.
    /// Nu reads those as written, never as a value, so not even `( )` is one.
    VariableName,
    /// An argument of an external command: a `[ ]` is a list.
    /// A `{ }` is a record or a block, as where nu takes any value.
    /// A `...` before a `$`, a `( )`, or a `[ ]` makes a spread, as in a list.
    ExternalArgument,
    /// A block or a closure: the body of a keyword syntax, what `else` takes, what an arm does.
    /// Nu reads a `{ }` there as a record only for a `:` second.
    /// One with a spread first is a block, unlike where nu takes any value.
    Block,
    /// The body of a `def`, which nu reads as a block without looking at what the `{ }` holds.
    DefBody,
}

enum Braced {
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

    fn reads_list(self) -> bool {
        matches!(self, Shape::Any | Shape::Block | Shape::ExternalArgument)
    }

    fn reads_table(self) -> bool {
        self.takes_any_value()
    }

    fn takes_any_value(self) -> bool {
        matches!(self, Shape::Any | Shape::Block)
    }

    fn braced(self, sign: Option<RecordSign>) -> Braced {
        match (self, sign) {
            (Shape::Any | Shape::ExternalArgument, Some(sign))
            | (Shape::KnownArgument | Shape::Block, Some(sign @ RecordSign::ColonSecond)) => {
                Braced::Record(sign)
            },
            (Shape::KnownArgument, Some(RecordSign::SpreadFirst | RecordSign::Empty))
            | (Shape::RowCondition, Some(_))
            | (Shape::UnknownArgument | Shape::VariableName, _) => Braced::Word,
            (
                Shape::Any | Shape::ExternalArgument | Shape::KnownArgument | Shape::RowCondition,
                None,
            )
            | (Shape::Block, None | Some(RecordSign::SpreadFirst | RecordSign::Empty))
            | (Shape::DefBody, _) => Braced::Block,
        }
    }
}

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
    } else if let Some(enclosed) = p.enclosed(&CURLY) {
        braced(p, enclosed, shape);
    } else if shape.takes_value() && p.peek() == Some(SyntaxKind::InterpString) {
        interpolation(p);
    } else if let Some(error) = variable_error(p, shape) {
        p.bump_error(error);
    } else {
        p.bump();
    }
}

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

/// `bracketed` is what it holds in list mode.
/// A list of items of another type holds no table, and its items take no `[ ]` or `{ }`.
fn listed<'a>(p: &mut Parser<'a>, bracketed: Bracketed<'a>, param: Param) {
    let list = is_list(bracketed.inner());
    let (kind, items): (_, fn(&mut Parser<'a>)) = match param.takes {
        Takes::List if list => (SyntaxKind::List, list_items),
        Takes::List if is_table(bracketed.inner()) => (SyntaxKind::Table, table_items),
        Takes::ListOf if list => (SyntaxKind::List, |p| items_as(p, Shape::UnknownArgument)),
        Takes::List | Takes::ListOf | Takes::Word => {
            p.bump();
            return;
        },
    };
    p.bump_bracketed(kind, bracketed, items);
}

fn braced<'a>(p: &mut Parser<'a>, enclosed: Enclosed<'a>, shape: Shape) {
    match shape.braced(record_sign(enclosed.tokens_in(Mode::Brace))) {
        Braced::Record(sign) => record(p, enclosed.lexed_in(Mode::Record), sign),
        Braced::Block => block(p, enclosed.lexed_in(Mode::Block)),
        Braced::Word => p.bump(),
    }
}

fn argument_braced<'a>(p: &mut Parser<'a>, enclosed: Enclosed<'a>, shape: Shape, param: Param) {
    match record_sign(enclosed.tokens_in(Mode::Brace)) {
        Some(RecordSign::Empty) if param.closure => block(p, enclosed.lexed_in(Mode::Block)),
        Some(RecordSign::Empty) => record(p, enclosed.lexed_in(Mode::Record), RecordSign::Empty),
        _ => braced(p, enclosed, shape),
    }
}

fn keyword_words(p: &Parser<'_>) -> usize {
    1 + usize::from(p.peek_text() == Some("export"))
}

/// The keyword ahead that declares a variable, and how many words it takes up.
/// Of the keywords that declare one, only `const` takes an `export` before it.
fn declaration<'a>(p: &Parser<'a>) -> Option<(&'a str, usize)> {
    let words = keyword_words(p);
    let keyword = p
        .nth_text(words - 1)
        .filter(|keyword| builtin::takes_variable_name(keyword))?;
    (words == 1 || keyword == "const").then_some((keyword, words))
}

/// Nu reads no flag past a `--`.
fn leading_flags(items: &[&str]) -> usize {
    let end = items
        .iter()
        .position(|item| !is_flag(item))
        .unwrap_or(items.len());
    items[..end]
        .iter()
        .position(|&item| item == "--")
        .map_or(end, |at| at + 1)
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

fn at_call_item(p: &Parser<'_>) -> bool {
    p.peek().is_some_and(|kind| !ends_call(kind))
}

fn call_items<'a>(p: &Parser<'a>) -> Vec<&'a str> {
    p.remaining_tokens()
        .filter(|(token, _)| !p.is_skipped(token.kind))
        .take_while(|(token, _)| !ends_call(token.kind))
        .map(|(_, text)| text)
        .collect()
}

/// Whether nu reads `text` as a flag, where no parameter takes a number.
fn is_flag(text: &str) -> bool {
    text.starts_with('-') && text.len() > 1
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
    use nuten_syntax::{SyntaxKind, SyntaxNode};

    use crate::parse;

    #[test]
    fn brackets_nested_too_deep_stay_word() {
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
    fn records_nested_too_deep_stay_word() {
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

    fn parsed_with(src: &str, kind: SyntaxKind) -> (SyntaxNode, usize) {
        let parsed = parse(src);
        assert!(parsed.errors().is_empty());
        let tree = parsed.syntax();
        let count = tree
            .descendants()
            .filter(|node| node.kind() == kind)
            .count();
        (tree, count)
    }

    fn innermost(tree: &SyntaxNode, kind: SyntaxKind) -> String {
        let innermost = tree
            .descendants()
            .filter(|node| node.kind() == kind)
            .last()
            .unwrap();
        assert!(innermost.children().next().is_none());
        innermost.to_string()
    }

    #[test]
    fn assignments_chained_too_deep_stay_as_written() {
        let src = format!("mut a = 1\n{}1", "$a = ".repeat(10_000));
        let (tree, assignments) = parsed_with(&src, SyntaxKind::Assignment);
        assert_eq!(assignments, 128);
        assert_eq!(
            innermost(&tree, SyntaxKind::Call),
            format!("{}1", "$a = ".repeat(10_000 - 128))
        );
    }

    #[test]
    fn declarations_chained_too_deep_stay_as_written() {
        let src = format!("{}1", "let a = ".repeat(10_000));
        let (tree, declarations) = parsed_with(&src, SyntaxKind::Let);
        assert_eq!(declarations, 128);
        assert_eq!(
            innermost(&tree, SyntaxKind::Call),
            format!("{}1", "let a = ".repeat(10_000 - 128))
        );
    }

    #[test]
    fn keyword_terms_chained_too_deep_stay_as_written() {
        let src = format!("{}1", "1 + if true {} else ".repeat(10_000));
        let (tree, ifs) = parsed_with(&src, SyntaxKind::If);
        assert_eq!(ifs, 128);
        assert_eq!(
            innermost(&tree, SyntaxKind::MathExpression),
            format!("{}1", "1 + if true {} else ".repeat(10_000 - 128))
        );
    }

    #[test]
    fn conditions_nested_too_deep_stay_as_written() {
        let src = format!("{}true{}", "if 1 + ".repeat(10_000), " {}".repeat(10_000));
        let (tree, ifs) = parsed_with(&src, SyntaxKind::If);
        assert_eq!(ifs, 128);
        let rest = 10_000 - 128;
        assert_eq!(
            innermost(&tree, SyntaxKind::MathExpression),
            format!("1 + {}true{}", "if 1 + ".repeat(rest), " {}".repeat(rest))
        );
    }

    #[test]
    fn blocks_nested_too_deep_stay_word() {
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
}
