//! Grammar rules of Nushell.

mod assignment;
mod block;
mod command;
mod control;
mod def;
mod if_;
mod list;
mod match_;
mod pipeline;
mod record;
mod signature;
mod table;

use nuten_syntax::SyntaxKind;

use self::{
    block::block,
    list::{is_list, list_items},
    pipeline::pipeline,
    record::{RecordSign, record, record_sign},
    table::{is_table, table_items},
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
            _ => pipeline(p, false),
        }
    }
}

/// What nu takes at a position, as far as it decides whether a word in
/// brackets is read as a value or kept whole. Nu's `SyntaxShape` decides
/// more; these are the ones that tell here.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Shape {
    /// Any value: `[ ]` makes a list or a table, `{ }` a record or a block.
    Any,
    /// An argument of a command whose signature is unknown. A `[ ]` stays a
    /// word, since a command may take it as a string. Nu reads a `{ }` as a
    /// string for a command it does not know and as a value for one it
    /// knows, which the signature is to tell; until then one holding a
    /// record stays a word and one holding a block is a block, as they were.
    Argument,
    /// An argument of an external command: a `[ ]` is a list, and a `{ }`
    /// a string.
    ExternalArgument,
    /// A block or a closure, as the body of a keyword syntax, what `else`
    /// takes, and what a `match` arm does. Nu reads a `{ }` there as a
    /// record only for a `:` second; one with a spread first is a block,
    /// unlike where nu takes any value.
    Block,
    /// The body of a `def`, which nu reads as a block without looking at
    /// what the `{ }` holds.
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
    /// Whether nu reads a `[ ]` as a list where it takes this shape.
    fn reads_list(self) -> bool {
        matches!(self, Shape::Any | Shape::Block | Shape::ExternalArgument)
    }

    /// Whether nu reads a `[ ]` as a table where it takes this shape.
    fn reads_table(self) -> bool {
        matches!(self, Shape::Any | Shape::Block)
    }

    /// What nu reads a `{ }` as where it takes this shape, given the
    /// `RecordSign` its first tokens show.
    fn braced(self, sign: Option<RecordSign>) -> Braced {
        match (self, sign) {
            (Shape::Any, Some(sign)) | (Shape::Block, Some(sign @ RecordSign::ColonSecond)) => {
                Braced::Record(sign)
            },
            (Shape::Argument, Some(_)) | (Shape::ExternalArgument, _) => Braced::Word,
            (Shape::Any | Shape::Argument, None)
            | (Shape::Block, None | Some(RecordSign::SpreadFirst))
            | (Shape::DefBody, _) => Braced::Block,
        }
    }
}

/// An item that is no shell operator, which nu rejects, as `value` reads it.
fn item(p: &mut Parser<'_>, shape: Shape) {
    match p.peek_text().and_then(shell_operator) {
        Some(kind) => p.bump_error(kind),
        None => value(p, shape),
    }
}

pub(crate) const SQUARE: Brackets = Brackets {
    open: b'[',
    close: b']',
    open_kind: SyntaxKind::LBracket,
    close_kind: SyntaxKind::RBracket,
    always_nested: true,
};

const CURLY: Brackets = Brackets {
    open: b'{',
    close: b'}',
    open_kind: SyntaxKind::LBrace,
    close_kind: SyntaxKind::RBrace,
    always_nested: true,
};

/// The next token as the value nu reads where it takes `shape`: a `List`,
/// a `Table`, a `Record`, or a `Block` where the shape has nu read one, or
/// the token as it is.
fn value(p: &mut Parser<'_>, shape: Shape) {
    if shape.reads_list()
        && let Some(bracketed) = p.bracketed(&SQUARE, Mode::list())
    {
        if is_list(bracketed.inner()) {
            p.bump_bracketed(SyntaxKind::List, bracketed, list_items);
        } else if shape.reads_table() && is_table(bracketed.inner()) {
            p.bump_bracketed(SyntaxKind::Table, bracketed, table_items);
        } else {
            p.bump();
        }
    } else if let Some(bracketed) = p.bracketed(&CURLY, Mode::brace()) {
        braced(p, bracketed, shape);
    } else {
        p.bump();
    }
}

/// The `{ }` ahead as what nu reads where it takes `shape`: a `Record`, a
/// `Block`, or the word as it is. `bracketed` is what it holds in brace
/// mode, which tells a record from the rest.
fn braced<'a>(p: &mut Parser<'a>, bracketed: Bracketed<'a>, shape: Shape) {
    match shape.braced(record_sign(bracketed.inner().remaining_tokens())) {
        Braced::Record(sign) => record(p, bracketed.lexed_in(Mode::record()), sign),
        Braced::Block => block(p, bracketed.lexed_in(Mode::block())),
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

fn is_item(kind: SyntaxKind) -> bool {
    matches!(
        kind,
        SyntaxKind::Word
            | SyntaxKind::String
            | SyntaxKind::RawString
            | SyntaxKind::InterpString
            | SyntaxKind::Error
    )
}

#[cfg(test)]
mod tests {
    use nuten_syntax::SyntaxKind;

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
}
