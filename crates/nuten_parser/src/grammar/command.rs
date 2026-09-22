use nuten_syntax::SyntaxKind;

use super::{
    Shape, builtin, ends_call, ends_element, is_file_redirection, item, keyword::keyword,
    shell_operator, value,
};
use crate::{ParseErrorKind, parser::Parser};

/// A `Call`, an `ExternalCall`, or a keyword syntax such as an `If`, then its arguments.
/// A value where the head would be is an expression, which no node holds.
/// Returns the streams it redirects; `None` when there is no command.
pub(super) fn command(
    p: &mut Parser<'_>,
    heads_pipeline: bool,
    assigning: bool,
) -> Option<Streams> {
    let head = Head::of(p, heads_pipeline);
    match head {
        Head::Missing => {},
        Head::Expression => item(p, Shape::Any),
        Head::Call(shape) => {
            if !keyword(p) {
                call(p, shape);
            }
        },
    }
    arguments(p, head.redirected(), head.shape(), assigning)
}

/// What stands where the head of a call would.
#[derive(Clone, Copy)]
enum Head {
    /// A file redirection or an assignment operator, which leaves the element with no command.
    Missing,
    /// A value, which nu reads as the start of an expression.
    Expression,
    /// A head, with the shape nu reads the items after it as.
    Call(Shape),
}

impl Head {
    fn of(p: &Parser<'_>, heads_pipeline: bool) -> Head {
        match p.peek() {
            Some(kind) if is_file_redirection(kind) || kind == SyntaxKind::Assign => Head::Missing,
            _ if p.peek_text().is_some_and(is_math_expression_like) => Head::Expression,
            _ => Head::Call(argument_shape(p, heads_pipeline)),
        }
    }

    /// The shape of what follows the head, which is an argument only after the head of a call.
    fn shape(self) -> Shape {
        match self {
            Head::Call(shape) => shape,
            Head::Missing | Head::Expression => Shape::UnknownArgument,
        }
    }

    /// `None` when the element has no command, which is what a redirection would apply to.
    fn redirected(self) -> Option<Streams> {
        match self {
            Head::Missing => None,
            Head::Expression | Head::Call(_) => Some(Streams::default()),
        }
    }
}

/// Whether nu reads `text` as the start of an expression, not as the head of a call.
/// Ported from nu's `is_math_expression_like`.
///
/// True only where nu reads an expression too: a call read as one loses the shape of its items.
/// A filesize, a duration, a date, a binary, and a range are left out, so each stays a head.
/// So are `if` and `match`, which `keyword` reads as their own syntax instead.
fn is_math_expression_like(text: &str) -> bool {
    matches!(text, "true" | "false" | "null" | "not")
        || text.starts_with("r#")
        || text.starts_with(['(', '{', '[', '$', '"', '\'', '-'])
        || is_number(text)
}

/// Whether nu's `parse_number` reads `text` as a number.
/// Nu drops every `_` before reading one.
fn is_number(text: &str) -> bool {
    let text = text.replace('_', "");
    let prefixed = [("0b", 2), ("0o", 8), ("0x", 16)]
        .into_iter()
        .find_map(|(prefix, radix)| Some((text.strip_prefix(prefix)?, radix)));
    match prefixed {
        Some((digits, radix)) => u64::from_str_radix(digits, radix).is_ok(),
        None => text.parse::<f64>().is_ok(),
    }
}

/// Nu runs a head it finds no declaration for as an external command.
/// Whether the file declares one is unknowable here, so an unlisted head stays unknown instead.
/// A `^` skips the lookup and a `%` looks past any alias, so both come before the alias check.
/// Nu runs `else` and `catch` externally wherever `if` and `try` have not taken them.
/// Nu reads a `@` as an attribute only where a pipeline starts.
fn argument_shape(p: &Parser<'_>, heads_pipeline: bool) -> Shape {
    let Some(head) = p.peek_text() else {
        return Shape::UnknownArgument;
    };
    if head.starts_with('^') {
        Shape::ExternalArgument
    } else if let Some(name) = head.strip_prefix('%') {
        percent_shape(name)
    } else if p.is_aliased(head) {
        Shape::UnknownArgument
    } else if matches!(head, "else" | "catch") {
        Shape::ExternalArgument
    } else if heads_pipeline && head.starts_with('@') {
        Shape::KnownArgument
    } else {
        command_shape(head)
    }
}

/// Nu runs a head whose `%` finds no built-in as an external command.
/// A `%` alone leaves the head in the token after it, which is not read here.
fn percent_shape(name: &str) -> Shape {
    if name.is_empty() {
        Shape::UnknownArgument
    } else if builtin::resolves_after_percent(name) {
        command_shape(name)
    } else {
        Shape::ExternalArgument
    }
}

fn command_shape(name: &str) -> Shape {
    if builtin::takes_variable_name(name) {
        Shape::UnknownArgument
    } else if builtin::takes_row_condition(name) {
        Shape::RowCondition
    } else if builtin::is_command(name) {
        Shape::KnownArgument
    } else {
        Shape::UnknownArgument
    }
}

/// The head, which nu reads as any value, then the arguments as `shape`.
fn call(p: &mut Parser<'_>, shape: Shape) {
    p.start(if shape == Shape::ExternalArgument {
        SyntaxKind::ExternalCall
    } else {
        SyntaxKind::Call
    });
    item(p, Shape::Any);
    while p.peek().is_some_and(|kind| !ends_call(kind)) {
        item(p, shape);
    }
    p.finish();
}

/// Items as `shape` and `Redirection`s up to the end of the element.
/// Up to an assignment operator instead when `assigning`.
/// `redirected` is what the element redirects so far; `None` when it has no command.
/// Returns what it redirects then.
pub(super) fn arguments(
    p: &mut Parser<'_>,
    mut redirected: Option<Streams>,
    shape: Shape,
    assigning: bool,
) -> Option<Streams> {
    while let Some(kind) = p.peek() {
        match kind {
            kind if ends_element(kind) => break,
            SyntaxKind::Assign if assigning => break,
            kind if is_file_redirection(kind) => {
                redirection(p, kind, redirected.as_mut());
                if p.peek() == Some(SyntaxKind::Assign)
                    && p.last_kind().is_some_and(is_file_redirection)
                {
                    p.bump();
                }
            },
            _ => item(p, shape),
        }
    }
    redirected
}

/// `redirected` is what the element redirects so far.
/// `None` when it has no command for the redirection to apply to.
///
/// Nu wants an item right after the operator.
/// Anything else is a missing target and stays outside the node.
fn redirection(p: &mut Parser<'_>, kind: SyntaxKind, redirected: Option<&mut Streams>) {
    p.start(SyntaxKind::Redirection);
    let has_target = p.nth(1).is_some_and(SyntaxKind::is_item)
        && p.nth_text(1).and_then(shell_operator).is_none();
    let streams = Streams::redirected_by(kind);
    let error = match redirected {
        None => Some(ParseErrorKind::RedirectionAtHead),
        Some(_) if !has_target => Some(ParseErrorKind::MissingRedirectionTarget),
        Some(redirected) if redirected.overlaps(streams) => {
            Some(ParseErrorKind::MultipleRedirections)
        },
        Some(redirected) => {
            redirected.add(streams);
            None
        },
    };
    match error {
        Some(error) => p.bump_error(error),
        None => p.bump(),
    }
    if has_target {
        value(p, Shape::Any);
    }
    p.finish();
}

/// Stdout and stderr, as what an operator redirects or an element has redirected.
#[derive(Debug, Clone, Copy, Default)]
pub(super) struct Streams {
    out: bool,
    err: bool,
}

impl Streams {
    pub(super) fn redirected_by(kind: SyntaxKind) -> Streams {
        let (out, err) = match kind {
            SyntaxKind::OutGt | SyntaxKind::OutGtGt => (true, false),
            SyntaxKind::ErrGt | SyntaxKind::ErrGtGt | SyntaxKind::ErrGtPipe => (false, true),
            SyntaxKind::OutErrGt | SyntaxKind::OutErrGtGt | SyntaxKind::OutErrGtPipe => {
                (true, true)
            },
            _ => unreachable!("{kind:?} is not a redirection"),
        };
        Streams {
            out,
            err,
        }
    }

    pub(super) fn overlaps(self, other: Streams) -> bool {
        (self.out && other.out) || (self.err && other.err)
    }

    fn add(&mut self, other: Streams) {
        self.out |= other.out;
        self.err |= other.err;
    }
}

#[cfg(test)]
mod tests {
    use super::is_math_expression_like;
    use crate::{ParseErrorKind, parse};

    fn errors(src: &str) -> Vec<ParseErrorKind> {
        parse(src).errors().iter().map(|error| error.kind).collect()
    }

    #[test]
    fn items_that_start_a_value_are_math_expression_like() {
        for text in [
            "true", "false", "null", "not", "r#'a'#", "(ls)", "{a: 1}", "[1]", "$env", "\"a\"",
            "'a'", "--flag", "1", "1_000", "1.5", "1e5", "inf", "0x10", "0o7", "0b1_0",
        ] {
            assert!(is_math_expression_like(text), "{text:?}");
        }
    }

    #[test]
    fn items_that_head_a_call_are_not_math_expression_like() {
        for text in ["ls", "rm", "nu-highlight", "1abc", "0x", "0xZZ", "_", ""] {
            assert!(!is_math_expression_like(text), "{text:?}");
        }
    }

    #[test]
    fn values_left_out_of_the_port_stay_heads() {
        for text in ["1kb", "1sec", "2024-01-01", "0x[ff]", "1..3", "if", "match"] {
            assert!(!is_math_expression_like(text), "{text:?}");
        }
    }

    #[test]
    fn redirection_target_is_missing_at_eof() {
        assert_eq!(errors("ls o>"), [ParseErrorKind::MissingRedirectionTarget]);
    }

    #[test]
    fn redirection_gets_one_error_at_most() {
        assert_eq!(errors("o>"), [ParseErrorKind::RedirectionAtHead]);
        assert_eq!(
            errors("ls o> a o> | x"),
            [ParseErrorKind::MissingRedirectionTarget]
        );
    }
}
