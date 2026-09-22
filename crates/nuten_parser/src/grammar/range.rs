use std::ops::Range;

use nuten_syntax::SyntaxKind;
use rowan::{TextRange, TextSize};

use super::{
    PAREN, Shape,
    cell_path::cell_path_ahead,
    number::is_number,
    value,
    variable::{is_variable_name, reads_dollar},
};
use crate::{
    lexer::Mode,
    parser::{Parser, Piece, Relexed},
};

/// What nu reads the word ahead as, where it reads a range.
pub(super) enum RangeAhead<'a> {
    /// The range, cut into its values and operators.
    Pieces(Relexed<'a>),
    /// A range with more of the word past its last operator, which nu leaves out, as in `1...`.
    /// It stays whole, since no piece of the range holds what nu leaves out.
    Whole,
}

/// Which values nuten takes for those of a range nu reads.
#[derive(Clone, Copy, PartialEq, Eq)]
enum ValuesTaken {
    /// Numbers, and a `$` or a `( )` that `value` reads as nu does, with no error.
    Read,
    /// Numbers alone.
    Numbers,
}

/// The range nu reads the word ahead as where it takes `shape`, if any.
/// Nu tries a range on a word with a `$` or a `( )` first wherever it reads a value.
/// It tries one on another word only where it takes any value.
/// It reads the one item after `else` or `=>` as the head of an expression, though.
/// A head is a range only where each value reads without an error.
/// For a `$` or a `( )`, that depends on what is in scope, so only numbers are taken there.
pub(super) fn range_ahead<'a>(p: &Parser<'a>, shape: Shape) -> Option<RangeAhead<'a>> {
    let text = p.peek_text()?;
    let taken = match text.as_bytes() {
        // Nu reads a cell path literal, a `[ ]`, a `{ }`, and a raw string as such, with no range.
        [b'$', b'.', ..] | [b'[' | b'{', ..] | [b'r', b'#', ..] => return None,
        [b'$', ..] => reads_dollar(text, shape).then_some(ValuesTaken::Read)?,
        [b'(', ..] => shape.takes_value().then_some(ValuesTaken::Read)?,
        _ => match shape {
            Shape::Any => ValuesTaken::Read,
            Shape::Block => ValuesTaken::Numbers,
            _ => return None,
        },
    };
    let start = p.peek_range()?.start();
    let in_source = |range: Range<usize>| absolute(start, range);
    let operators = operators(text)?;
    let first_operator = operators.next.unwrap_or(operators.at);
    // Nu lexes what comes before the first operator before it reads a value.
    // The value checks below reject every word this step rejects, but the port keeps nu's step.
    if first_operator > 0
        && p.lex_error(in_source(0..first_operator), Mode::RangeFrom)
            .is_some()
    {
        return None;
    }
    let values = operators.values(text)?;
    if !values
        .ranges()
        .all(|range| reads_value(p, in_source(range), taken))
    {
        return None;
    }
    if operators.leaves_tail(text) {
        return Some(RangeAhead::Whole);
    }
    let operator = |range: Range<usize>| Piece::Token(SyntaxKind::RangeOperator, in_source(range));
    let value = |range: Range<usize>| Piece::Lexed(in_source(range));
    let pieces: Vec<_> = values
        .first
        .map(value)
        .into_iter()
        .chain(operators.next.map(|next| operator(next..next + 2)))
        .chain(values.second.map(value))
        .chain([operator(operators.at..operators.at + operators.len)])
        .chain(values.last.map(value))
        .collect();
    p.split(&pieces).map(RangeAhead::Pieces)
}

/// The range `range_ahead` found: its values as `value` reads them, with the operators among them.
pub(super) fn range<'a>(p: &mut Parser<'a>, ahead: RangeAhead<'a>) {
    match ahead {
        RangeAhead::Pieces(pieces) => p.bump_relexed(SyntaxKind::Range, pieces, |p| {
            while let Some(kind) = p.peek() {
                if kind == SyntaxKind::RangeOperator {
                    p.bump();
                } else {
                    value(p, Shape::Any);
                }
            }
        }),
        RangeAhead::Whole => p.bump(),
    }
}

/// Whether nu reads `text` as a range with numbers alone as its values.
pub(super) fn is_number_range(text: &str) -> bool {
    operators(text)
        .and_then(|operators| operators.values(text))
        .is_some_and(|values| values.ranges().all(|range| is_number(&text[range])))
}

/// Where nu's `parse_range` finds the operators of a range in a word.
struct Operators {
    /// The `..` before the second value, if any.
    next: Option<usize>,
    /// The operator before the last value.
    at: usize,
    /// The length of that operator: 2 for `..`, 3 for `..=` and `..<`.
    len: usize,
}

/// The operators nu's `parse_range` finds in `text`, if it goes on to read the values.
/// It counts a `..` only where the `(` before it are as many as the `)`, those in strings too.
/// It takes a `..<` only as the first one in `text`.
fn operators(text: &str) -> Option<Operators> {
    if text.starts_with("...") {
        return None;
    }
    let dotdots: Vec<_> = text
        .match_indices("..")
        .map(|(at, _)| at)
        .filter(|&at| {
            let before = &text[..at];
            before
                .matches('(')
                .count()
                .checked_sub(before.matches(')').count())
                == Some(0)
        })
        .collect();
    let (next, at) = match dotdots[..] {
        [at] => (None, at),
        [next, at] => (Some(next), at),
        _ => return None,
    };
    let len = match text.find("..<") {
        Some(exclusive) if exclusive == at => 3,
        Some(_) => return None,
        None if text[at..].starts_with("..=") => 3,
        None => 2,
    };
    Some(Operators {
        next,
        at,
        len,
    })
}

impl Operators {
    /// Where the values of `text` are.
    /// Nu takes no first value when `text` starts with `..`.
    /// It takes no last value when `text` ends with the spelling of the last operator.
    /// `None` when it takes neither, or when the second value is empty.
    fn values(&self, text: &str) -> Option<Values> {
        let end = self.at + self.len;
        let first = (!text.starts_with("..")).then(|| 0..self.next.unwrap_or(self.at));
        let last = (!text.ends_with(&text[self.at..end])).then_some(end..text.len());
        if first.is_none() && last.is_none() {
            return None;
        }
        let second = match self.next {
            Some(next) if next + 2 == self.at => return None,
            Some(next) => Some(next + 2..self.at),
            None => None,
        };
        Some(Values {
            first,
            second,
            last,
        })
    }

    /// Whether `text` goes on past the last operator where nu takes no last value.
    fn leaves_tail(&self, text: &str) -> bool {
        let end = self.at + self.len;
        end < text.len() && text.ends_with(&text[self.at..end])
    }
}

/// Where the values of a range are in its word.
struct Values {
    first: Option<Range<usize>>,
    second: Option<Range<usize>>,
    last: Option<Range<usize>>,
}

impl Values {
    fn ranges(&self) -> impl Iterator<Item = Range<usize>> {
        [&self.first, &self.second, &self.last]
            .into_iter()
            .flatten()
            .cloned()
    }
}

/// Whether nu reads the text `range` covers as a value of a range without an error of its own.
/// Nu reads no range where a value reads with an error, so a word holding one stays whole.
/// Nu reads a record and other values too, then reports that no range takes their type.
/// Nuten does not check types, so those stay in the word.
fn reads_value(p: &Parser<'_>, range: TextRange, taken: ValuesTaken) -> bool {
    let Some(piece) = p.item_at(range) else {
        return false;
    };
    let text = piece.peek_text().expect("an item at the range");
    match (taken, text.as_bytes()) {
        (ValuesTaken::Read, [b'$', b'"' | b'\'' | b'.', ..]) => false,
        (ValuesTaken::Read, [b'$', ..]) => reads_variable(&piece),
        (ValuesTaken::Read, [b'(', ..]) => {
            piece.bracketed(&PAREN, Mode::Paren).is_some() || path_rejects_nothing(&piece)
        },
        _ => piece.peek() == Some(SyntaxKind::Word) && is_number(text),
    }
}

/// Whether nu reads the `$` ahead, a variable and any path into it, without an error.
fn reads_variable(piece: &Parser<'_>) -> bool {
    let Some(relexed) = piece.relexed(Mode::CellPath) else {
        return false;
    };
    let tokens: Vec<_> = relexed.inner().remaining_tokens().collect();
    match tokens.as_slice() {
        [(_, head), path @ ..] => {
            is_variable_name(&head[1..]) && (path.is_empty() || path_rejects_nothing(piece))
        },
        [] => false,
    }
}

/// Whether nu reads a path after the value ahead, rejecting none of it.
fn path_rejects_nothing(piece: &Parser<'_>) -> bool {
    cell_path_ahead(piece, Shape::Any).is_some_and(|ahead| ahead.rejects_nothing())
}

fn absolute(start: TextSize, range: Range<usize>) -> TextRange {
    let offset = |at: usize| start + TextSize::try_from(at).expect("an offset within a word");
    TextRange::new(offset(range.start), offset(range.end))
}

#[cfg(test)]
mod tests {
    use super::operators;

    /// The operators, as `(next, at, len)`, that nu's `parse_range` finds.
    fn found(text: &str) -> Option<(Option<usize>, usize, usize)> {
        operators(text).map(|operators| (operators.next, operators.at, operators.len))
    }

    #[test]
    fn operators_are_where_nu_finds_them() {
        for (text, expected) in [
            ("1..2", (None, 1, 2)),
            ("1..=2", (None, 1, 3)),
            ("1..<2", (None, 1, 3)),
            ("1..2..3", (Some(1), 4, 2)),
            ("1..2..<3", (Some(1), 4, 3)),
            ("..2..3", (Some(0), 3, 2)),
            ("1...3", (None, 1, 2)),
            ("(1..2)..3", (None, 6, 2)),
        ] {
            assert_eq!(found(text), Some(expected), "{text:?}");
        }
    }

    #[test]
    fn operators_are_none_where_nu_reads_no_range() {
        for text in [
            "1",
            "...3",
            "1..2..3..4",
            "1..<2..3",
            "(1..<2)..3",
            "1..(2..<3)",
            "(\"(\")..3",
            "a)..3",
        ] {
            assert!(found(text).is_none(), "{text:?}");
        }
    }

    #[test]
    fn values_are_what_nu_takes_between_operators() {
        for (text, expected) in [
            ("1..2", (Some(0..1), None, Some(3..4))),
            ("..2", (None, None, Some(2..3))),
            ("1..", (Some(0..1), None, None)),
            ("1..=", (Some(0..1), None, None)),
            ("1..2..", (Some(0..1), Some(3..4), None)),
            ("..2..3", (None, Some(2..3), Some(5..6))),
            ("1...", (Some(0..1), None, None)),
        ] {
            let values = operators(text).unwrap().values(text).unwrap();
            assert_eq!(
                (values.first, values.second, values.last),
                expected,
                "{text:?}"
            );
        }
    }

    #[test]
    fn range_without_first_and_last_value_or_with_empty_second_has_no_values() {
        for text in ["..", "..=", "1....3", "..<"] {
            let values = operators(text).and_then(|operators| operators.values(text));
            assert!(values.is_none(), "{text:?}");
        }
    }

    #[test]
    fn only_range_with_more_past_last_operator_that_no_value_takes_leaves_tail() {
        for (text, tail) in [
            ("1...", true),
            ("1..2...", true),
            ("1..", false),
            ("1..3", false),
            ("1...3", false),
        ] {
            assert_eq!(operators(text).unwrap().leaves_tail(text), tail, "{text:?}");
        }
    }
}
