use nuten_syntax::SyntaxKind;

use super::{
    Braced, CURLY, PAREN, RecordSign, SQUARE, Shape, number::int, record_sign, value,
    variable::reads_dollar,
};
use crate::{
    ParseErrorKind,
    lexer::{Mode, Token},
    parser::{Parser, Relexed},
};

/// The word ahead split as nu splits a full cell path: the value, then the path into it.
pub(super) struct CellPathAhead<'a> {
    relexed: Relexed<'a>,
    /// Whether it is a cell path literal, as in `$.a`, whose `$` nu reads as no value.
    literal: bool,
    /// The links of the path up to the first one nu rejects, after which nu reads no more of it.
    links: Vec<Link>,
}

/// A token of the path, as nu's `parse_cell_path` reads it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Link {
    /// A `.`, `?`, or `!` where nu takes one.
    Separator,
    /// A member, which nu reads as an int or a string.
    Member,
    /// A token nu rejects where it stands, with its error.
    Rejected(ParseErrorKind),
}

impl CellPathAhead<'_> {
    /// Whether nu reads the path to its end, rejecting none of it.
    pub(super) fn rejects_nothing(&self) -> bool {
        !self
            .links
            .iter()
            .any(|link| matches!(link, Link::Rejected(_)))
    }
}

/// The cell path ahead, when nu reads the word as a value and a path where it takes `shape`.
/// The word stays whole where nuten does not read it as nu does yet.
pub(super) fn cell_path_ahead<'a>(p: &Parser<'a>, shape: Shape) -> Option<CellPathAhead<'a>> {
    let text = p.peek_text()?;
    if !text.starts_with(['$', '(', '[', '{']) || !text.contains(['.', '?', '!']) {
        return None;
    }
    let relexed = p.relexed(Mode::CellPath)?;
    let tokens: Vec<_> = relexed.inner().remaining_tokens().collect();
    let (&(_, head_text), path) = tokens.split_first()?;
    if path.is_empty() || !reads_head(relexed.inner(), head_text, shape) {
        return None;
    }
    let literal = text.starts_with("$.");
    let links = links(path)?;
    Some(CellPathAhead {
        relexed,
        literal,
        links,
    })
}

/// Whether nu reads `head`, which `inner` starts with, as the value of a cell path for `shape`.
/// Nu reads a `[ ]` there only where it takes any value.
/// It reads a `{ }` there only as a record with a `:` second, or an empty one.
/// Each bracket must close at the last byte of `head`.
fn reads_head(inner: &Parser<'_>, head: &str, shape: Shape) -> bool {
    match head.as_bytes() {
        [b'$', ..] => reads_dollar(head, shape),
        [b'(', ..] => shape.takes_value() && inner.bracketed(&PAREN, Mode::Paren).is_some(),
        [b'[', ..] => shape.takes_any_value() && inner.bracketed(&SQUARE, Mode::List).is_some(),
        [b'{', ..] => inner.bracketed(&CURLY, Mode::Brace).is_some_and(|braced| {
            let sign = record_sign(braced.inner().remaining_tokens());
            matches!(
                shape.braced(sign),
                Braced::Record(RecordSign::ColonSecond | RecordSign::Empty)
            )
        }),
        _ => false,
    }
}

/// What nu takes next in a path, as its `parse_cell_path` tracks it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Expect {
    /// Only `.`, as right after the value or after both `?` and `!`.
    Dot,
    /// `.`, `?`, or `!`, as right after a member.
    DotOrSign,
    /// `.` or `!`, as after a `?`.
    DotOrBang,
    /// `.` or `?`, as after a `!`.
    DotOrQuestion,
    /// A member, as after a `.`.
    Member,
}

impl Expect {
    /// What nu makes of the token of `kind` and `text` here.
    /// `None` for a member whose error is not checked here.
    fn read(self, kind: SyntaxKind, text: &str) -> Option<Link> {
        if self == Expect::Member {
            return member(text);
        }
        Some(match self.after(kind) {
            Some(_) => Link::Separator,
            None => Link::Rejected(ParseErrorKind::CellPathSeparator {
                expected: self.separators(),
            }),
        })
    }

    /// What nu takes next after a token of `kind` here; `None` for a separator it rejects.
    fn after(self, kind: SyntaxKind) -> Option<Expect> {
        match (self, kind) {
            (Expect::Member, _) => Some(Expect::DotOrSign),
            (_, SyntaxKind::Dot) => Some(Expect::Member),
            (Expect::DotOrSign, SyntaxKind::Question) => Some(Expect::DotOrBang),
            (Expect::DotOrSign, SyntaxKind::Bang) => Some(Expect::DotOrQuestion),
            (Expect::DotOrBang, SyntaxKind::Bang)
            | (Expect::DotOrQuestion, SyntaxKind::Question) => Some(Expect::Dot),
            _ => None,
        }
    }

    /// The separators nu says it takes, where it takes no member.
    fn separators(self) -> &'static str {
        match self {
            Expect::Dot => "`.`",
            Expect::DotOrSign => "`.`, `?`, or `!`",
            Expect::DotOrBang => "`.` or `!`",
            Expect::DotOrQuestion => "`.` or `?`",
            Expect::Member => unreachable!("a member is never rejected as a separator"),
        }
    }
}

/// The links of `path`, the tokens after the value, up to the first one nu rejects.
/// `None` for a comment among them, which nu drops.
/// The formatter would print a comment inside the node away from where it stands.
/// `None` too for a member whose error is not checked here.
/// `None` too for a token with a lexing error, which the word read whole holds none of.
fn links(path: &[(Token, &str)]) -> Option<Vec<Link>> {
    if path
        .iter()
        .any(|(token, _)| matches!(token.kind, SyntaxKind::Comment | SyntaxKind::Error))
    {
        return None;
    }
    path.iter()
        .scan(Some(Expect::Dot), |state, &(token, text)| {
            let expect = (*state)?;
            let link = expect.read(token.kind, text);
            *state = match link {
                Some(Link::Separator | Link::Member) => expect.after(token.kind),
                Some(Link::Rejected(_)) | None => None,
            };
            Some(link)
        })
        .collect()
}

/// How nu reads `text` as a member: an int that is not negative, or else a string.
/// `None` for a `"` string with a `\`, since whether nu takes its escapes is not checked here.
fn member(text: &str) -> Option<Link> {
    if let Some(int) = int(text) {
        return Some(if int < 0 {
            Link::Rejected(ParseErrorKind::NegativeCellPathIndex)
        } else {
            Link::Member
        });
    }
    match text.as_bytes() {
        [b'"', .., b'"'] if text.contains('\\') => None,
        [b'"', .., b'"'] | [b'\'', .., b'\''] | [b'`', ..] => Some(Link::Member),
        // Nu takes nothing after the closing quote of a `'` or `"` string.
        [b'"' | b'\'', ..] => Some(Link::Rejected(ParseErrorKind::CellPathMember)),
        // Nu reads a member with a `(` and no quote first as an interpolation, which is no string.
        _ if text.contains('(') => Some(Link::Rejected(ParseErrorKind::CellPathMember)),
        _ => Some(Link::Member),
    }
}

/// The cell path that `cell_path_ahead` found, with any value as `value` reads it for `shape`.
/// A member lexed as neither a word nor a string is a `Word`, since nu reads it as a plain string.
pub(super) fn cell_path<'a>(p: &mut Parser<'a>, ahead: CellPathAhead<'a>, shape: Shape) {
    let CellPathAhead {
        relexed,
        literal,
        links,
    } = ahead;
    p.bump_relexed(SyntaxKind::CellPath, relexed, |p| {
        if literal {
            p.bump();
        } else {
            value(p, shape);
        }
        for link in links {
            match link {
                Link::Member
                    if !matches!(p.peek(), Some(SyntaxKind::Word | SyntaxKind::String)) =>
                {
                    p.bump_as(SyntaxKind::Word);
                },
                Link::Rejected(error) => p.bump_error(error),
                Link::Separator | Link::Member => p.bump(),
            }
        }
        while p.peek().is_some() {
            p.bump();
        }
    });
}

#[cfg(test)]
mod tests {
    use nuten_syntax::SyntaxKind;

    use super::{Link, member};
    use crate::{ParseErrorKind, parse};

    #[test]
    fn string_or_non_negative_int_is_member() {
        for text in [
            "a",
            "0",
            "-0",
            "9223372036854775808",
            "a-b",
            "a#b",
            "?",
            "$'a'",
            "r#'a'#",
            "\"a b\"",
            "\"(a)\"",
            "'(a)'",
            "'a\\q'",
            "'a'b'c'",
            "\"a\"b\"c\"",
            "`(a)`",
            "`a`b",
            "`a`(b)",
        ] {
            assert_eq!(member(text), Some(Link::Member), "{text:?}");
        }
    }

    #[test]
    fn negative_int_member_is_rejected() {
        for text in ["-1", "-_1", "0xffffffffffffffff"] {
            assert_eq!(
                member(text),
                Some(Link::Rejected(ParseErrorKind::NegativeCellPathIndex)),
                "{text:?}"
            );
        }
    }

    #[test]
    fn non_string_member_is_rejected() {
        for text in ["a(b)", "(a)", "$'(a)'", "r#'(a)'#", "'a'b", "\"a\"b"] {
            assert_eq!(
                member(text),
                Some(Link::Rejected(ParseErrorKind::CellPathMember)),
                "{text:?}"
            );
        }
    }

    #[test]
    fn backslash_in_double_quoted_member_is_unchecked() {
        for text in ["\"a\\n\"", "\"a\\q\""] {
            assert_eq!(member(text), None, "{text:?}");
        }
    }

    fn cell_paths(src: &str) -> usize {
        parse(src)
            .syntax()
            .descendants()
            .filter(|node| node.kind() == SyntaxKind::CellPath)
            .count()
    }

    #[test]
    fn unchecked_word_nu_rejects_stays_whole_without_error() {
        for src in [
            "let $x.a = 1",
            "let (x).a = 1",
            "let $.a = 1",
            "^echo [1 2].0",
            "{...$r}.a",
            "{a}.b",
            "$\"a\".b",
            "$'a'.b",
            "$'a'-b",
            "[a]b.c",
        ] {
            assert_eq!(cell_paths(src), 0, "{src:?}");
            assert!(parse(src).errors().is_empty(), "{src:?}");
        }
    }

    /// A `#` after a vertical tab or a form feed starts a comment that runs to the end of the word.
    /// Lexed again as a path, it runs only to the next `.`, which leaves a `)` unmatched.
    #[test]
    fn path_with_lexing_error_stays_whole() {
        for src in ["$\u{b}#.)[", "$\u{c}#.)", "$\u{b}#..)"] {
            assert_eq!(cell_paths(src), 0, "{src:?}");
        }
    }

    #[test]
    fn path_error_leaves_enclosing_block_whole() {
        let parsed = parse("if true { $x? }");
        assert!(parsed.errors().is_empty());
        let words = parsed
            .syntax()
            .descendants_with_tokens()
            .filter(|element| element.kind() == SyntaxKind::Word);
        assert_eq!(
            words.map(|word| word.to_string()).collect::<Vec<_>>(),
            ["if", "true", "{ $x? }"]
        );
    }

    #[test]
    fn paths_nested_too_deep_stay_word() {
        let src = format!("{}ls{}", "(".repeat(130), ").a".repeat(130));
        assert_eq!(cell_paths(&src), 128);
    }
}
