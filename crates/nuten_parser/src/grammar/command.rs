use nuten_syntax::SyntaxKind;

use super::{
    Shape, ends_call, ends_element, is_file_redirection, is_item, item, keyword::keyword,
    shell_operator, value,
};
use crate::{ParseErrorKind, parser::Parser};

/// A `Call`, an `ExternalCall`, or a keyword syntax such as an `If`, then its arguments.
/// Returns the streams it redirects; `None` when there is no command.
pub(super) fn command(p: &mut Parser<'_>, assigning: bool) -> Option<Streams> {
    let has_call = !p
        .peek()
        .is_some_and(|kind| is_file_redirection(kind) || kind == SyntaxKind::Assign);
    // Nu runs `else` and `catch` at the start of a statement as external commands.
    let external = p
        .peek_text()
        .is_some_and(|head| head.starts_with('^') || matches!(head, "else" | "catch"));
    let shape = if external {
        Shape::ExternalArgument
    } else {
        Shape::KnownArgument
    };
    if has_call && !keyword(p) {
        call(p, shape);
    }
    arguments(p, has_call.then_some(Streams::default()), shape, assigning)
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
    let has_target =
        p.nth(1).is_some_and(is_item) && p.nth_text(1).and_then(shell_operator).is_none();
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
    use crate::{ParseErrorKind, parse};

    fn errors(src: &str) -> Vec<ParseErrorKind> {
        parse(src).errors().iter().map(|error| error.kind).collect()
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
