use nuten_syntax::SyntaxKind;

use super::{
    Shape, is_file_redirection, is_pipe,
    spread::{Target, is_spread, spread},
    value,
};
use crate::{ParseErrorKind, parser::Parser};

/// A `;` may make it a table, or else a word.
/// A lexing error keeps it a word too, since nu lexed the word whole without one.
pub(super) fn is_list(inner: &Parser<'_>) -> bool {
    !inner
        .remaining_kinds()
        .any(|kind| matches!(kind, SyntaxKind::Semicolon | SyntaxKind::Error))
}

/// Unlike `item`, reports no shell operator.
pub(super) fn list_items(p: &mut Parser<'_>) {
    piped_items(p, |p| match p.peek_text() {
        Some(text) if is_spread(text, Target::List) => spread(p, Target::List),
        _ => value(p, Shape::Any),
    });
}

/// The items of a list whose items nu reads as another type than any value, each as `shape`.
/// A spread stays a word, which reads the same as its value would.
pub(super) fn items_as(p: &mut Parser<'_>, shape: Shape) {
    piped_items(p, |p| value(p, shape));
}

/// The items of a list, each read by `item`, and the pipes among them.
/// Nu's lite parser reads what a list holds as a pipeline.
pub(super) fn piped_items<'a>(p: &mut Parser<'a>, mut item: impl FnMut(&mut Parser<'a>)) {
    // An attribute heading the list takes in the rest of it, pipes too.
    // Pipes beside a file redirection stay items too, as nuten does not read what nu drops.
    let pipes_separate = !headed_by_attribute(p) && !p.remaining_kinds().any(is_file_redirection);
    while let Some(kind) = p.peek() {
        if is_pipe(kind) {
            pipe(p, kind, pipes_separate);
        } else {
            item(p);
        }
    }
}

fn pipe(p: &mut Parser<'_>, kind: SyntaxKind, pipes_separate: bool) {
    match kind {
        // Nu rejects it whatever else the list holds.
        SyntaxKind::Pipe if pipe_ends_list(p) => p.bump_error(ParseErrorKind::PipeEndsList),
        // An assignment operator takes in the rest of the list, pipes too.
        _ if !pipes_separate || p.emitted_kinds().any(|kind| kind == SyntaxKind::Assign) => {
            p.bump();
        },
        SyntaxKind::Pipe => p.bump(),
        _ if follows_item(p) => p.bump(),
        _ => p.bump_error(ParseErrorKind::RedirectionAtHead),
    }
}

/// Nu's lite parser reads an attribute only at the start of a pipeline.
fn headed_by_attribute(p: &Parser<'_>) -> bool {
    p.remaining_tokens()
        .find(|(token, _)| !reads_as_space(token.kind))
        .is_some_and(|(_, text)| text.starts_with('@'))
}

/// Whether the `|` ahead is the last token of the list.
fn pipe_ends_list(p: &Parser<'_>) -> bool {
    p.remaining_kinds()
        .filter(|&kind| !reads_as_space(kind))
        .nth(1)
        .is_none()
}

/// Whether an item stands between the last pipe, or the start of the list, and the token ahead.
fn follows_item(p: &Parser<'_>) -> bool {
    p.emitted_kinds()
        .find(|&kind| !reads_as_space(kind))
        .is_some_and(|kind| !is_pipe(kind))
}

/// Nu's lexer reads a `,` in a list as a space.
fn reads_as_space(kind: SyntaxKind) -> bool {
    kind.is_trivia() || kind == SyntaxKind::Comma
}

#[cfg(test)]
mod tests {
    use crate::{ParseErrorKind, parse};

    fn errors(src: &str) -> Vec<ParseErrorKind> {
        parse(src).errors().iter().map(|error| error.kind).collect()
    }

    #[test]
    fn pipe_ending_list_reports_pipe_ends_list() {
        assert_eq!(errors("[a |]"), [ParseErrorKind::PipeEndsList]);
        assert_eq!(errors("[|]"), [ParseErrorKind::PipeEndsList]);
        assert_eq!(errors("[a | | ]"), [ParseErrorKind::PipeEndsList]);
        assert_eq!(errors("[a |,]"), [ParseErrorKind::PipeEndsList]);
        assert_eq!(errors("[a |\r\n]"), [ParseErrorKind::PipeEndsList]);
        assert_eq!(errors("[a | # c\n]"), [ParseErrorKind::PipeEndsList]);
        assert_eq!(errors("[a e>| |]"), [ParseErrorKind::PipeEndsList]);
        assert_eq!(errors("[a = b |]"), [ParseErrorKind::PipeEndsList]);
        assert_eq!(errors("[@a |]"), [ParseErrorKind::PipeEndsList]);
        assert_eq!(errors("[a o> b |]"), [ParseErrorKind::PipeEndsList]);
    }

    #[test]
    fn redirecting_pipe_without_item_before_reports_redirection_at_head() {
        assert_eq!(errors("[e>| a]"), [ParseErrorKind::RedirectionAtHead]);
        assert_eq!(errors("[\no+e>| a]"), [ParseErrorKind::RedirectionAtHead]);
        assert_eq!(errors("[, e>| a]"), [ParseErrorKind::RedirectionAtHead]);
        assert_eq!(errors("[a | e>| b]"), [ParseErrorKind::RedirectionAtHead]);
        assert_eq!(
            errors("[a | # c\ne>| b]"),
            [ParseErrorKind::RedirectionAtHead]
        );
        assert_eq!(errors("[a e>| e>| b]"), [ParseErrorKind::RedirectionAtHead]);
    }

    #[test]
    fn plain_pipe_not_ending_list_or_redirecting_pipe_after_item_has_no_error() {
        assert_eq!(errors("[| a]"), []);
        assert_eq!(errors("[a | | b]"), []);
        assert_eq!(errors("[a e>| b]"), []);
        assert_eq!(errors("[a,\ne>| b]"), []);
        assert_eq!(errors("[a e>|]"), []);
        assert_eq!(errors("[a o+e>|]"), []);
    }

    #[test]
    fn pipe_past_assignment_operator_or_in_list_headed_by_attribute_has_no_error() {
        assert_eq!(errors("[= e>| a]"), []);
        assert_eq!(errors("[a = | e>| b]"), []);
        assert_eq!(errors("[@a e>| b]"), []);
        assert_eq!(errors("[, @a | e>| b]"), []);
        assert_eq!(errors("[# c\n@a\n| e>| b]"), []);
    }
}
