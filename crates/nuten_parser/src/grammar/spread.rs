use nuten_syntax::SyntaxKind;
use rowan::{TextRange, TextSize};

use super::{
    CURLY, SQUARE, Shape,
    cell_path::{cell_path, cell_path_ahead},
    record, record_sign, squared, value,
};
use crate::{
    lexer::Mode,
    parser::{Parser, Piece},
};

/// What a spread goes into, which tells the value nu takes after its `...`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Target {
    /// A list, or the arguments of a command.
    List,
    Record,
}

impl Target {
    /// The bracket of a value this takes after `...`, on top of a `$` or a `(`.
    fn bracket(self) -> char {
        match self {
            Target::List => '[',
            Target::Record => '{',
        }
    }
}

/// Whether nu reads `text` as a spread into `target`.
/// Ported from nu's `extract_spread_list` and `extract_spread_record`.
pub(super) fn is_spread(text: &str, target: Target) -> bool {
    text.strip_prefix("...")
        .is_some_and(|rest| rest.starts_with(['$', '(', target.bracket()]))
}

/// The spread ahead, which `is_spread` takes, as a `Spread`: the `...`, then the value.
/// A word where the value is not one item.
pub(super) fn spread(p: &mut Parser<'_>, target: Target) {
    let range = p.peek_range().expect("a spread ahead");
    let operator_end = range.start() + TextSize::of("...");
    let pieces = [
        Piece::Token(
            SyntaxKind::SpreadOperator,
            TextRange::new(range.start(), operator_end),
        ),
        Piece::Lexed(TextRange::new(operator_end, range.end())),
    ];
    match p.split(&pieces) {
        Some(split) => p.bump_relexed(SyntaxKind::Spread, split, |p| {
            p.bump();
            spread_value(p, target);
        }),
        None => p.bump(),
    }
}

/// The value after `...` as nu reads it where it takes a list or a record.
/// Unlike where it takes any value, nu reads no cell path after a `[ ]`.
/// It reads a `{ }` with the shape of a record, so one that is no record stays a word.
fn spread_value(p: &mut Parser<'_>, target: Target) {
    match (target, p.peek_text().map(str::as_bytes)) {
        (Target::List, Some([b'[', ..])) => match p.bracketed(&SQUARE, Mode::List) {
            Some(bracketed) => squared(p, bracketed, Shape::Any),
            None => p.bump(),
        },
        (Target::Record, Some([b'{', ..])) => {
            if let Some(ahead) = cell_path_ahead(p, Shape::Any) {
                cell_path(p, ahead, Shape::Any);
            } else if let Some(bracketed) = p.bracketed(&CURLY, Mode::Brace)
                && let Some(sign) = record_sign(bracketed.inner().remaining_tokens())
            {
                record(p, bracketed.lexed_in(Mode::Record), sign);
            } else {
                p.bump();
            }
        },
        _ => value(p, Shape::Any),
    }
}

#[cfg(test)]
mod tests {
    use super::{Target, is_spread};

    #[test]
    fn spread_is_three_dots_then_value_target_takes() {
        for (text, target) in [
            ("...$x", Target::List),
            ("...(f)", Target::List),
            ("...[1]", Target::List),
            ("...$x", Target::Record),
            ("...(f)", Target::Record),
            ("...{a: 1}", Target::Record),
        ] {
            assert!(is_spread(text, target), "{text:?} {target:?}");
        }
        for (text, target) in [
            ("...{a: 1}", Target::List),
            ("...[1]", Target::Record),
            ("...a", Target::List),
            ("...", Target::List),
            ("....$x", Target::List),
            ("..$x", Target::List),
            ("$x", Target::List),
        ] {
            assert!(!is_spread(text, target), "{text:?} {target:?}");
        }
    }
}
