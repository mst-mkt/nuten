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

pub(super) enum RangeAhead<'a> {
    Pieces(Relexed<'a>),
    /// No piece of the range holds what nu leaves out past its last operator.
    Whole,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ValuesTaken {
    Read,
    Numbers,
}

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

pub(super) fn is_number_range(text: &str) -> bool {
    operators(text)
        .and_then(|operators| operators.values(text))
        .is_some_and(|values| values.ranges().all(|range| is_number(&text[range])))
}

struct Operators {
    next: Option<usize>,
    at: usize,
    len: usize,
}

/// Ported from nu's `parse_range`.
/// It counts a `..` only where the `(` before it are as many as the `)`, those in strings too.
/// It takes a `..<` only as the first one in `text`.
fn operators(text: &str) -> Option<Operators> {
    // Most words hold no `.`, and searching one for `..` sets up a searcher each time.
    if !text.contains('.') || text.starts_with("...") {
        return None;
    }
    let mut dotdots = balanced_dotdots(text);
    let (next, at) = match (dotdots.next(), dotdots.next(), dotdots.next()) {
        (Some(at), None, _) => (None, at),
        (Some(next), Some(at), None) => (Some(next), at),
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

/// Each count goes on from the `..` before, since counting from the start for each is quadratic.
fn balanced_dotdots(text: &str) -> impl Iterator<Item = usize> {
    text.match_indices("..")
        .scan((0, 0), |(counted, depth), (at, _)| {
            *depth += text[*counted..at]
                .bytes()
                .map(|byte| match byte {
                    b'(' => 1,
                    b')' => -1,
                    _ => 0,
                })
                .sum::<isize>();
            *counted = at;
            Some((at, *depth))
        })
        .filter(|&(_, depth)| depth == 0)
        .map(|(at, _)| at)
}

impl Operators {
    /// Nu takes no last value when `text` ends with the spelling of the last operator.
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

    fn leaves_tail(&self, text: &str) -> bool {
        let end = self.at + self.len;
        end < text.len() && text.ends_with(&text[self.at..end])
    }
}

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
    fn non_range_has_no_operators() {
        for text in [
            "1",
            "1.5",
            "$a.b",
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
    fn values_are_where_nu_takes_them() {
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
    fn bare_operators_or_empty_second_have_no_values() {
        for text in ["..", "..=", "1....3", "..<"] {
            let values = operators(text).and_then(|operators| operators.values(text));
            assert!(values.is_none(), "{text:?}");
        }
    }

    #[test]
    fn only_untaken_text_past_last_operator_leaves_tail() {
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
