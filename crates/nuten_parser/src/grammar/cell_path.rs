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

pub(super) struct CellPathAhead<'a> {
    relexed: Relexed<'a>,
    /// Nu reads the `$` of a cell path literal such as `$.a` as no value.
    literal: bool,
    /// Nu reads no more of the path past the first link it rejects.
    links: Vec<Link>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Link {
    Separator,
    Member,
    Rejected(ParseErrorKind),
}

impl CellPathAhead<'_> {
    pub(super) fn rejects_nothing(&self) -> bool {
        !self
            .links
            .iter()
            .any(|link| matches!(link, Link::Rejected(_)))
    }
}

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

fn reads_head(inner: &Parser<'_>, head: &str, shape: Shape) -> bool {
    match head.as_bytes() {
        [b'$', ..] => reads_dollar(head, shape),
        [b'(', ..] => shape.takes_value() && inner.bracketed(&PAREN, Mode::Paren).is_some(),
        [b'[', ..] => shape.takes_any_value() && inner.bracketed(&SQUARE, Mode::List).is_some(),
        [b'{', ..] => inner.enclosed(&CURLY).is_some_and(|enclosed| {
            let sign = record_sign(enclosed.tokens_in(Mode::Brace));
            matches!(
                shape.braced(sign),
                Braced::Record(RecordSign::ColonSecond | RecordSign::Empty)
            )
        }),
        _ => false,
    }
}

/// Ported from how nu's `parse_cell_path` tracks what it takes next.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Expect {
    Dot,
    DotOrSign,
    DotOrBang,
    DotOrQuestion,
    Member,
}

impl Expect {
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

/// `None` for a comment among them, which nu drops but the formatter would move.
/// `None` for a token with a lexing error, which the word read whole holds none of.
/// `None` for a member whose error is not checked here.
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
