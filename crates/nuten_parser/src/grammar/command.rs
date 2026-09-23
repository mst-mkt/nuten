use nuten_syntax::SyntaxKind;

use super::{
    Shape, at_call_item,
    binary::is_binary_like,
    builtin, call_items, declaration, ends_element, is_file_redirection, item,
    keyword::{keyword, keyword_as_term},
    leading_flags,
    number::is_number,
    range::is_number_range,
    shell_operator, value,
};
use crate::{ParseErrorKind, parser::Parser};

/// A `Call`, an `ExternalCall`, or a keyword syntax such as an `If`, then its arguments.
/// A value or a `not` where the head would be is an expression: a `MathExpression`, or the value.
/// Returns the streams it redirects; `None` when there is no command.
pub(super) fn command(p: &mut Parser<'_>, assigning: bool) -> Option<Streams> {
    let head = Head::of(p);
    match head {
        Head::Missing => {},
        Head::Expression => expression(p),
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
    /// A value or a `not`, which nu reads as an expression of its own.
    Expression,
    /// A head, with the shape nu reads the items after it as.
    Call(Shape),
}

impl Head {
    fn of(p: &Parser<'_>) -> Head {
        match p.peek() {
            Some(kind) if is_file_redirection(kind) || kind == SyntaxKind::Assign => Head::Missing,
            _ if p.peek_text().is_some_and(is_math_expression_like) => Head::Expression,
            _ => Head::Call(argument_shape(p)),
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

/// The items nu reads as an expression, as a `MathExpression` where it takes an operator.
/// An `if` or a `match` first heads a call over them instead, as in nu's `parse_math_expression`.
pub(super) fn expression(p: &mut Parser<'_>) {
    if keyword_as_term(p) {
        return;
    }
    if takes_operator(&call_items(p)) {
        math_expression(p);
    } else {
        item(p, Shape::Any);
    }
}

/// The `len` items ahead as nu reads the condition of an `if` or a `while`.
/// Nu reads the flags first as those of the keyword, then the rest as one expression.
/// The items after the condition are other arguments, so the expression takes none of them.
pub(super) fn condition(p: &mut Parser<'_>, len: usize) {
    let flags = leading_flags(&call_items(p)[..len]);
    for _ in 0..flags {
        p.bump();
    }
    if flags < len {
        let end = p
            .nth_range(len - flags - 1)
            .expect("a condition ahead")
            .end();
        p.bump_bounded(end, expression);
    }
}

/// Whether `expression` reads the items as a `MathExpression`, where nu takes an operator.
/// Nu reads the first item as a value and takes an operator after it.
/// A `not` is an operator itself, and takes a term even as the only item.
fn takes_operator(items: &[&str]) -> bool {
    matches!(items, ["not"] | [_, _, ..])
}

/// The terms an operator joins, as nu's `parse_math_expression` reads them.
fn math_expression(p: &mut Parser<'_>) {
    p.start(SyntaxKind::MathExpression);
    term(p);
    while at_call_item(p) {
        joined_term(p);
    }
    p.finish();
}

/// The operator ahead and the term it joins to what came before.
/// An item where nu takes an operator stays as it is written, since nu reads no value there.
fn joined_term(p: &mut Parser<'_>) {
    // An `Error` token carries the error of its own text, so none is reported over it.
    if p.peek() == Some(SyntaxKind::Error) {
        return p.bump();
    }
    if !p.peek_text().is_some_and(is_operator) {
        let error = p
            .peek_text()
            .and_then(|text| shell_operator(text).or_else(|| unknown_operator(text)))
            .unwrap_or(ParseErrorKind::MissingOperator);
        return p.bump_error(error);
    }
    if !operator(p) {
        return;
    }
    // A keyword syntax holds the rest of the element, so it leaves the loop no item to join.
    // A `not` before it takes that word as a value instead, as `term` does.
    if !keyword_as_term(p) {
        term(p);
    }
}

/// The `not`s nu reads as prefix operators, then the value they take.
fn term(p: &mut Parser<'_>) {
    while p.peek_text() == Some("not") {
        if !operator(p) {
            return;
        }
    }
    item(p, Shape::Any);
}

/// The operator ahead, or the error where the element holds no term for it.
/// Returns whether the term it takes is the next item to read.
fn operator(p: &mut Parser<'_>) -> bool {
    if !takes_term(p) {
        p.bump_error(ParseErrorKind::MissingTerm);
        return false;
    }
    p.bump_as(SyntaxKind::Operator);
    at_call_item(p)
}

/// Whether the element holds a term for the operator ahead.
/// Nu lifts a redirection and the item it takes out of the element, so a term may stand past them.
/// It stays where it is written, as any item after a redirection does.
fn takes_term(p: &Parser<'_>) -> bool {
    let mut at = 1;
    while let Some(kind) = p.nth(at) {
        if ends_element(kind) || kind == SyntaxKind::Assign {
            return false;
        }
        if !is_file_redirection(kind) {
            return true;
        }
        match p.nth(at + 1) {
            Some(target) if target.is_item() => at += 2,
            _ => return false,
        }
    }
    false
}

/// Whether nu's `parse_operator` reads `text` as an operator.
/// Its arms are in nu's order, so that the port can be checked against them.
/// The spellings it takes only to reject are left out; `unknown_operator` holds them.
/// So is an assignment operator, which the lexer gives a kind of its own.
/// So is `not`, which nu reads as a prefix of a term, though `term` emits it as an `Operator` too.
fn is_operator(text: &str) -> bool {
    matches!(
        text,
        "==" | "!="
            | "<"
            | "<="
            | ">"
            | ">="
            | "=~"
            | "like"
            | "!~"
            | "not-like"
            | "in"
            | "not-in"
            | "has"
            | "not-has"
            | "starts-with"
            | "not-starts-with"
            | "ends-with"
            | "not-ends-with"
            | "+"
            | "-"
            | "*"
            | "/"
            | "//"
            | "mod"
            | "**"
            | "++"
            | "bit-or"
            | "bit-xor"
            | "bit-and"
            | "bit-shl"
            | "bit-shr"
            | "or"
            | "xor"
            | "and"
    )
}

/// What nu says to write instead of `text`, among the spellings it takes only to reject.
/// Ported from the arms of nu's `parse_operator` that report an unknown operator, in their order.
fn unknown_operator(text: &str) -> Option<ParseErrorKind> {
    let (spelling, instead) = match text {
        "^" => ("^", "`**` to raise to a power, or `bit-xor` to xor bits"),
        "pow" => ("pow", "`**` to raise to a power, or `bit-xor` to xor bits"),
        "is" => ("is", "`==`"),
        "===" => ("===", "`==`"),
        "contains" => ("contains", "`has`"),
        "%" => ("%", "`mod`"),
        "&" => ("&", "`bit-and`"),
        "<<" => ("<<", "`bit-shl`"),
        ">>" => (">>", "`bit-shr`"),
        "bits-and" => ("bits-and", "`bit-and`"),
        "bits-xor" => ("bits-xor", "`bit-xor`"),
        "bits-or" => ("bits-or", "`bit-or`"),
        "bits-shl" => ("bits-shl", "`bit-shl`"),
        "bits-shr" => ("bits-shr", "`bit-shr`"),
        _ => return None,
    };
    Some(ParseErrorKind::UnknownOperator {
        spelling,
        instead,
    })
}

/// Whether nu reads `text` as the start of an expression, not as the head of a call.
/// Ported from nu's `is_math_expression_like`.
///
/// True only where nu reads an expression too: a call read as one loses the shape of its items.
/// A filesize, a duration, and a date are left out, so each stays a head.
/// So are `if` and `match`, which `keyword` reads as their own syntax instead.
/// So is a range with a value other than a number.
/// Nu reads it as an expression only where each value reads without an error.
/// For a `$` or a `( )`, that depends on what is in scope.
pub(super) fn is_math_expression_like(text: &str) -> bool {
    matches!(text, "true" | "false" | "null" | "not")
        || text.starts_with("r#")
        || text.starts_with(['(', '{', '[', '$', '"', '\'', '-'])
        || is_number(text)
        || is_binary_like(text)
        || is_number_range(text)
}

/// Nu runs a head it finds no declaration for as an external command.
/// Whether the file declares one is unknowable here, so an unlisted head stays unknown instead.
/// A `^` skips the lookup and a `%` looks past any alias, so both come before the alias check.
/// Nu runs `else` and `catch` externally wherever `if` and `try` have not taken them.
/// It reads the name after `let`, `mut`, `const`, or `export const` as written, never as a value.
pub(super) fn argument_shape(p: &Parser<'_>) -> Shape {
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
    } else if declaration(p).is_some() {
        Shape::VariableName
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
    if builtin::takes_any_argument(name) {
        Shape::Any
    } else if builtin::takes_row_condition(name) {
        Shape::RowCondition
    } else if builtin::is_command(name) {
        Shape::KnownArgument
    } else {
        Shape::UnknownArgument
    }
}

/// The head, then the arguments as `shape`.
pub(super) fn call(p: &mut Parser<'_>, shape: Shape) {
    start_call(p, shape);
    while at_call_item(p) {
        item(p, shape);
    }
    p.finish();
}

/// Starts the `Call`, or the `ExternalCall` where `shape` is external, with the head in it.
/// Nu reads the head as the name of a command, never as a value.
pub(super) fn start_call(p: &mut Parser<'_>, shape: Shape) {
    p.start(if shape == Shape::ExternalArgument {
        SyntaxKind::ExternalCall
    } else {
        SyntaxKind::Call
    });
    match p.peek_text().and_then(shell_operator) {
        Some(kind) => p.bump_error(kind),
        None => p.bump(),
    }
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
    use super::{is_math_expression_like, is_operator, takes_operator, unknown_operator};
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
    fn operators_are_the_spellings_nu_joins_terms_with() {
        for text in [
            "==",
            "!=",
            "<",
            "<=",
            ">",
            ">=",
            "=~",
            "like",
            "!~",
            "not-like",
            "in",
            "not-in",
            "has",
            "not-has",
            "starts-with",
            "not-starts-with",
            "ends-with",
            "not-ends-with",
            "+",
            "-",
            "*",
            "/",
            "//",
            "mod",
            "**",
            "++",
            "bit-or",
            "bit-xor",
            "bit-and",
            "bit-shl",
            "bit-shr",
            "or",
            "xor",
            "and",
        ] {
            assert!(is_operator(text), "{text:?}");
        }
    }

    #[test]
    fn spellings_nu_takes_only_to_reject_are_no_operator() {
        for text in [
            "^", "pow", "is", "===", "contains", "%", "&", "<<", ">>", "bits-and", "bits-or",
            "bits-xor", "bits-shl", "bits-shr",
        ] {
            assert!(!is_operator(text), "{text:?}");
        }
    }

    #[test]
    fn spellings_nu_takes_only_to_reject_say_what_to_write_instead() {
        for (text, instead) in [
            ("^", "`**` to raise to a power, or `bit-xor` to xor bits"),
            ("pow", "`**` to raise to a power, or `bit-xor` to xor bits"),
            ("is", "`==`"),
            ("===", "`==`"),
            ("contains", "`has`"),
            ("%", "`mod`"),
            ("&", "`bit-and`"),
            ("<<", "`bit-shl`"),
            (">>", "`bit-shr`"),
            ("bits-and", "`bit-and`"),
            ("bits-xor", "`bit-xor`"),
            ("bits-or", "`bit-or`"),
            ("bits-shl", "`bit-shl`"),
            ("bits-shr", "`bit-shr`"),
        ] {
            let expected = ParseErrorKind::UnknownOperator {
                spelling: text,
                instead,
            };
            assert_eq!(unknown_operator(text), Some(expected), "{text:?}");
        }
    }

    #[test]
    fn spellings_nu_never_takes_for_an_operator_have_nothing_to_write_instead() {
        for text in [
            "1", "$a", "ls", "not", "+", "and", "bit-and", "mod", "", "||", "&&", "2>", "2>&1",
            "o>|", "out>|",
        ] {
            assert!(unknown_operator(text).is_none(), "{text:?}");
        }
    }

    #[test]
    fn assignment_operators_are_no_operator() {
        for text in ["=", "+=", "++=", "-=", "*=", "/="] {
            assert!(!is_operator(text), "{text:?}");
        }
    }

    #[test]
    fn items_and_shell_operators_are_no_operator() {
        for text in ["1", "$a", "ls", "-2", "bit", "", "||", "&&", "2>&1"] {
            assert!(!is_operator(text), "{text:?}");
        }
    }

    #[test]
    fn not_is_no_operator_that_joins_two_terms() {
        assert!(!is_operator("not"));
    }

    #[test]
    fn items_past_the_first_take_an_operator() {
        assert!(takes_operator(&["1", "2"]));
        assert!(takes_operator(&["1", "+", "2"]));
    }

    #[test]
    fn not_is_an_operator_even_as_the_only_item() {
        assert!(takes_operator(&["not"]));
        assert!(takes_operator(&["not", "$a"]));
        assert!(takes_operator(&["not", "not", "$a"]));
    }

    #[test]
    fn fewer_than_two_items_without_a_not_take_no_operator() {
        for items in [["1"].as_slice(), &[]] {
            assert!(!takes_operator(items), "{items:?}");
        }
    }

    #[test]
    fn values_left_out_of_the_port_stay_heads() {
        for text in ["1kb", "1sec", "2024-01-01", "if", "match"] {
            assert!(!is_math_expression_like(text), "{text:?}");
        }
    }

    #[test]
    fn range_with_variable_or_subexpression_as_value_stays_head() {
        for text in ["1..$x", "1..(2)", "..$x", "1..2..$x"] {
            assert!(!is_math_expression_like(text), "{text:?}");
        }
    }

    #[test]
    fn binary_nu_decodes_or_reports_is_math_expression_like() {
        for text in ["0x[ff]", "0o[777]", "0x[zz]"] {
            assert!(is_math_expression_like(text), "{text:?}");
        }
        assert!(!is_math_expression_like("0x[ff|00]"));
    }

    #[test]
    fn range_of_numbers_alone_is_math_expression_like() {
        for text in ["1..3", "..5", "1..", "0x10..=1e3", "1..2..<10", "1..."] {
            assert!(is_math_expression_like(text), "{text:?}");
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
