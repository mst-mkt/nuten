use nuten_syntax::SyntaxKind;

use super::{
    Shape, builtin, ends_statement, item,
    pipeline::{bump_newlines, next_line_starts_with_pipe, pipe_ends_file},
};
use crate::{ParseErrorKind, parser::Parser};

/// Whether an item that starts with `@` stands ahead.
pub(super) fn attribute_ahead(p: &Parser<'_>) -> bool {
    p.peek().zip(p.peek_text()).is_some_and(heads_attribute)
}

/// Whether the token of `kind` and `text` may head an attribute.
fn heads_attribute((kind, text): (SyntaxKind, &str)) -> bool {
    kind.is_item() && text.starts_with('@')
}

/// The `Attribute` ahead.
/// Nu looks up `@name` as the command `attr name`.
pub(super) fn attribute(p: &mut Parser<'_>) {
    let name = p.peek_text().map_or("", |head| &head[1..]);
    let shape = if builtin::is_attribute(name) {
        Shape::KnownArgument
    } else {
        Shape::UnknownArgument
    };
    p.start(SyntaxKind::Attribute);
    p.bump();
    attribute_arguments(p, shape);
    p.finish();
}

/// The items after the head of an attribute, as `shape`.
/// Nu's lite parser takes every token up to the end of the line as an argument, a `|` too.
/// Unlike after a `|` in a pipeline, a line break right after one ends the attribute.
/// Nu reports a `|` last in the file even so.
fn attribute_arguments(p: &mut Parser<'_>, shape: Shape) {
    while let Some(kind) = p.peek() {
        match kind {
            SyntaxKind::Newline if next_line_starts_with_pipe(p) => bump_newlines(p),
            kind if ends_statement(kind) => break,
            SyntaxKind::Pipe if pipe_ends_file(p) => p.bump_error(ParseErrorKind::PipeAtEof),
            kind if kind.is_item() => item(p, shape),
            _ => p.bump(),
        }
    }
}
