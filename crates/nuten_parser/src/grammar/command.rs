use nuten_syntax::SyntaxKind;

use super::{
    CURLY, Callee, SQUARE, Shape, argument_braced, at_call_item,
    binary::is_binary_like,
    builtin, call_items, declaration, ends_element, is_file_redirection, item,
    keyword::{keyword, keyword_as_term},
    leading_flags, listed,
    number::is_number,
    parameters::{Param, Takes, parameters},
    range::is_number_range,
    shell_operator, value,
};
use crate::{ParseErrorKind, lexer::Mode, parser::Parser};

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

/// The `items` ahead as nu reads the condition of an `if` or a `while`.
/// Nu reads the flags first as those of the keyword, then the rest as one expression.
/// The items after the condition are other arguments, so the expression takes none of them.
pub(super) fn condition(p: &mut Parser<'_>, items: &[&str]) {
    let flags = leading_flags(items);
    for _ in 0..flags {
        p.bump();
    }
    if flags < items.len() {
        let end = p
            .nth_range(items.len() - flags - 1)
            .expect("a condition ahead")
            .end();
        p.bump_bounded(end, expression);
    }
}

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

fn term(p: &mut Parser<'_>) {
    while p.peek_text() == Some("not") {
        if !operator(p) {
            return;
        }
    }
    item(p, Shape::Any);
}

/// Returns whether the term it takes is the next item to read.
fn operator(p: &mut Parser<'_>) -> bool {
    if !takes_term(p) {
        p.bump_error(ParseErrorKind::MissingTerm);
        return false;
    }
    p.bump_as(SyntaxKind::Operator);
    at_call_item(p)
}

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

fn is_operator(text: &str) -> bool {
    operator_precedence(text).is_some()
}

/// How tightly nu binds the operator `text`, as its `Operator::precedence` ranks it.
/// The smaller, the looser.
/// `None` where nu's `parse_operator` reads no operator.
/// Its arms are in the order of `parse_operator`, so that the port can be checked against them.
/// The spellings it takes only to reject are left out; `unknown_operator` holds them.
/// So is an assignment operator, which the lexer gives a kind of its own.
/// So is `not`, which nu reads as a prefix of a term, though `term` emits it as an `Operator` too.
#[expect(clippy::match_same_arms, reason = "the arms follow `parse_operator`")]
pub fn operator_precedence(text: &str) -> Option<u8> {
    Some(match text {
        "==" | "!=" | "<" | "<=" | ">" | ">=" | "=~" | "like" | "!~" | "not-like" | "in"
        | "not-in" | "has" | "not-has" | "starts-with" | "not-starts-with" | "ends-with"
        | "not-ends-with" => 80,
        "+" | "-" => 90,
        "*" | "/" | "//" | "mod" => 95,
        "**" => 100,
        "++" => 80,
        "bit-or" => 60,
        "bit-xor" => 70,
        "bit-and" => 75,
        "bit-shl" | "bit-shr" => 85,
        "or" => 40,
        "xor" => 45,
        "and" => 50,
        _ => return None,
    })
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
/// What an import nuten cannot read declares is unknowable here.
/// So a head that neither the lists here nor a block around it names stays unknown.
/// So does one after such an import, which may bring an alias of an external command in its name.
/// A `^` skips the lookup and a `%` looks past any alias, so both come before the alias check.
/// Nu runs `else` and `catch` externally wherever `if` and `try` have not taken them.
/// It reads the name after `let`, `mut`, `const`, or `export const` as written, never as a value.
/// A command a block around the call defines takes the place of any other of its name.
/// Nu reads an argument of one as it does one of its own commands, never as an external one's.
pub(super) fn argument_shape(p: &Parser<'_>) -> Shape {
    let Some(head) = p.peek_text() else {
        return Shape::UnknownArgument;
    };
    if head.starts_with('^') {
        Shape::ExternalArgument
    } else if let Some(name) = head.strip_prefix('%') {
        percent_shape(name, p.nth_text(1))
    } else if p.is_aliased(head) {
        Shape::UnknownArgument
    } else if declaration(p).is_some() {
        Shape::VariableName
    } else {
        match p.callee() {
            // Nu runs its own command of a longer name, as `take until` over a defined `take`.
            Callee::Defined
                if builtin::row_condition_words(head, p.nth_text(1)) > p.defined_words() =>
            {
                Shape::RowCondition
            },
            Callee::Defined => Shape::KnownArgument,
            Callee::Unknowable => Shape::UnknownArgument,
            Callee::Undefined if matches!(head, "else" | "catch") => Shape::ExternalArgument,
            Callee::Undefined => command_shape(head, p.nth_text(1)),
        }
    }
}

/// Nu runs a head whose `%` finds no built-in as an external command.
/// A `%` alone leaves the head in the token after it, which is not read here.
fn percent_shape(name: &str, next: Option<&str>) -> Shape {
    if name.is_empty() {
        Shape::UnknownArgument
    } else if builtin::resolves_after_percent(name) {
        command_shape(name, next)
    } else {
        Shape::ExternalArgument
    }
}

fn command_shape(name: &str, next: Option<&str>) -> Shape {
    if builtin::takes_any_argument(name) {
        Shape::Any
    } else if builtin::row_condition_words(name, next).is_some() {
        Shape::RowCondition
    } else if builtin::is_command(name) {
        Shape::KnownArgument
    } else {
        Shape::UnknownArgument
    }
}

/// Where the call runs a command nuten knows the signature of, a `[ ]` is what its parameter takes.
pub(super) fn call(p: &mut Parser<'_>, shape: Shape) {
    let items = match shape {
        Shape::ExternalArgument | Shape::UnknownArgument => Vec::new(),
        _ => call_items(p),
    };
    let params = call_parameters(p, shape, &items);
    let words = call_name_words(p, shape, &items);
    open_call(p, shape);
    if words > 1 {
        p.start(SyntaxKind::CommandName);
        head(p);
        for _ in 1..words {
            p.bump();
        }
        p.finish();
    } else {
        head(p);
    }
    for param in params.into_iter().skip(words) {
        if !at_call_item(p) {
            break;
        }
        argument(p, shape, param);
    }
    while at_call_item(p) {
        item(p, shape);
    }
    p.finish();
}

/// What nu takes each item of the call ahead as, the head first.
/// Empty where it runs a command the file defines with a signature nuten cannot read.
/// Empty too where nuten cannot tell which command it runs.
/// A `%` before the head looks up a command nu has, whatever the file defines.
fn call_parameters(p: &Parser<'_>, shape: Shape, items: &[&str]) -> Vec<Option<Param>> {
    if shape != Shape::KnownArgument {
        return Vec::new();
    }
    match p.callee() {
        Callee::Defined => p.defined_parameters(items).unwrap_or_default(),
        Callee::Unknowable => Vec::new(),
        Callee::Undefined => parameters(&as_looked_up(items)),
    }
}

/// `items`, with the head looked up as nu looks it up, past any `%`.
fn as_looked_up<'s>(items: &[&'s str]) -> Vec<&'s str> {
    items
        .iter()
        .enumerate()
        .map(|(index, item)| match index {
            0 => item.strip_prefix('%').unwrap_or(item),
            _ => item,
        })
        .collect()
}

fn argument(p: &mut Parser<'_>, shape: Shape, param: Option<Param>) {
    if let Some(param) = param
        && param.takes != Takes::Word
        && let Some(bracketed) = p.bracketed(&SQUARE, Mode::List)
    {
        listed(p, bracketed, param);
    } else if let Some(param) = param
        && let Some(enclosed) = p.enclosed(&CURLY)
    {
        // `value` would read such a word through `braced` too.
        // A word the brackets enclose is no range, cell path, binary, or variable.
        argument_braced(p, enclosed, shape, param);
    } else {
        item(p, shape);
    }
}

/// Starts the call and takes its head alone, for an alias value or what an attribute heads.
/// Neither is a call whose name nuten looks up.
pub(super) fn start_call(p: &mut Parser<'_>, shape: Shape) {
    open_call(p, shape);
    head(p);
}

fn open_call(p: &mut Parser<'_>, shape: Shape) {
    p.start(if shape == Shape::ExternalArgument {
        SyntaxKind::ExternalCall
    } else {
        SyntaxKind::Call
    });
}

/// Nu reads the head as the name of a command, never as a value.
fn head(p: &mut Parser<'_>) {
    match p.peek_text().and_then(shell_operator) {
        Some(kind) => p.bump_error(kind),
        None => p.bump(),
    }
}

/// How many of `items` name the command, as nu takes the longest name they spell.
fn call_name_words(p: &Parser<'_>, shape: Shape, items: &[&str]) -> usize {
    if matches!(shape, Shape::ExternalArgument | Shape::UnknownArgument) {
        return 1;
    }
    let builtin = builtin::name_words(&as_looked_up(items));
    match p.callee() {
        Callee::Defined => builtin.max(p.defined_words().unwrap_or(1)),
        Callee::Unknowable | Callee::Undefined => builtin,
    }
}

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
/// Nu wants an item right after the operator.
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
    use super::{
        is_math_expression_like, is_operator, operator_precedence, takes_operator, unknown_operator,
    };
    use crate::{ParseErrorKind, parse, test_support::asked};

    fn errors(src: &str) -> Vec<ParseErrorKind> {
        parse(src).errors().iter().map(|error| error.kind).collect()
    }

    #[test]
    fn value_starts_are_math_expression_like() {
        for text in [
            "true", "false", "null", "not", "r#'a'#", "(ls)", "{a: 1}", "[1]", "$env", "\"a\"",
            "'a'", "--flag", "1", "1_000", "1.5", "1e5", "inf", "0x10", "0o7", "0b1_0",
        ] {
            assert!(is_math_expression_like(text), "{text:?}");
        }
    }

    #[test]
    fn call_heads_are_not_math_expression_like() {
        for text in ["ls", "rm", "nu-highlight", "1abc", "0x", "0xZZ", "_", ""] {
            assert!(!is_math_expression_like(text), "{text:?}");
        }
    }

    /// The spellings nu's `parse_operator` reads as operators.
    const OPERATORS: &[&str] = &[
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
    ];

    /// For each two of the `$ops` operators, `L` where nu groups `1 a 2 b 3` as `(1 a 2) b 3`.
    /// `R` where it groups them as `1 a (2 b 3)`.
    const GROUPINGS_OF_NU: &str = r#"$ops | each {|a| $ops | each {|b|
            let expr = ast --json $"1 ($a) 2 ($b) 3" | get block | from json
                | get pipelines.0.elements.0.expr.expr
            let left = $expr.BinaryOp.0.expr
            let grouped = if 'BinaryOp' in ($left | columns) { 'L' } else { 'R' }
            $"($a) ($b) ($grouped)"
        } } | flatten | str join "\n""#;

    #[test]
    fn every_nu_operator_is_operator() {
        for text in OPERATORS {
            assert!(is_operator(text), "{text:?}");
        }
    }

    #[test]
    fn looser_operator_groups_what_tighter_one_joins() {
        let list = OPERATORS
            .iter()
            .map(|operator| format!("'{operator}'"))
            .collect::<Vec<_>>()
            .join(" ");
        let Some(groupings) = asked(&format!("let ops = [{list}]\n{GROUPINGS_OF_NU}")) else {
            return;
        };
        let left = |a: &str, b: &str| {
            let line = format!("{a} {b} L");
            groupings.contains(&line)
        };
        for a in OPERATORS {
            for b in OPERATORS {
                let (Some(first), Some(second)) = (operator_precedence(a), operator_precedence(b))
                else {
                    panic!("{a:?} {b:?} have no precedence");
                };
                // `**` groups to the right and the rest to the left.
                // So operators of equal precedence only have to group alike whichever comes first.
                let expected = if first == second {
                    left(b, a)
                } else {
                    first > second
                };
                assert_eq!(left(a, b), expected, "{a:?} {b:?}");
            }
        }
    }

    #[test]
    fn rejected_spellings_are_no_operator() {
        for text in [
            "^", "pow", "is", "===", "contains", "%", "&", "<<", ">>", "bits-and", "bits-or",
            "bits-xor", "bits-shl", "bits-shr",
        ] {
            assert!(!is_operator(text), "{text:?}");
        }
    }

    #[test]
    fn rejected_spellings_suggest_what_to_write() {
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
    fn other_spellings_suggest_nothing() {
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
    fn not_is_no_binary_operator() {
        assert!(!is_operator("not"));
    }

    #[test]
    fn items_past_first_take_operator() {
        assert!(takes_operator(&["1", "2"]));
        assert!(takes_operator(&["1", "+", "2"]));
    }

    #[test]
    fn not_takes_operator_even_alone() {
        assert!(takes_operator(&["not"]));
        assert!(takes_operator(&["not", "$a"]));
        assert!(takes_operator(&["not", "not", "$a"]));
    }

    #[test]
    fn lone_value_or_nothing_takes_no_operator() {
        for items in [["1"].as_slice(), &[]] {
            assert!(!takes_operator(items), "{items:?}");
        }
    }

    #[test]
    fn units_dates_and_keywords_stay_heads() {
        for text in ["1kb", "1sec", "2024-01-01", "if", "match"] {
            assert!(!is_math_expression_like(text), "{text:?}");
        }
    }

    #[test]
    fn range_with_variable_or_subexpression_stays_head() {
        for text in ["1..$x", "1..(2)", "..$x", "1..2..$x"] {
            assert!(!is_math_expression_like(text), "{text:?}");
        }
    }

    #[test]
    fn binary_like_is_math_expression_like() {
        for text in ["0x[ff]", "0o[777]", "0x[zz]"] {
            assert!(is_math_expression_like(text), "{text:?}");
        }
        assert!(!is_math_expression_like("0x[ff|00]"));
    }

    #[test]
    fn number_range_is_math_expression_like() {
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
