//! Grammar rules of Nushell.

use nuten_syntax::SyntaxKind;
use rowan::{TextRange, TextSize};

use crate::{
    ParseError, ParseErrorKind,
    lexer::Mode,
    parser::{Brackets, Parser},
};

pub(crate) fn file(p: &mut Parser<'_>) {
    while let Some(kind) = p.peek() {
        match kind {
            SyntaxKind::Semicolon if p.last_kind() == Some(SyntaxKind::Pipe) => {
                p.bump_error(ParseErrorKind::SemicolonAfterPipe);
            },
            kind if ends_statement(kind) => p.bump(),
            _ => pipeline(p, false),
        }
    }
}

/// `assigned` is whether an assignment operator takes the pipeline.
fn pipeline(p: &mut Parser<'_>, assigned: bool) {
    p.start(SyntaxKind::Pipeline);
    let mut element = None;
    let mut first = true;
    while let Some(kind) = p.peek() {
        match kind {
            kind if is_pipe(kind) => pipe(p, kind, element.take(), assigned),
            SyntaxKind::Newline if next_line_starts_with_pipe(p) => bump_newlines(p),
            kind if ends_statement(kind) => break,
            _ => {
                element = pipeline_element(p, first);
                first = false;
            },
        }
    }
    p.finish();
}

/// `element` is what the element before the pipe redirects;
/// `None` when it has no command. `assigned` is as for `pipeline`.
fn pipe(p: &mut Parser<'_>, kind: SyntaxKind, element: Option<Streams>, assigned: bool) {
    match kind {
        SyntaxKind::Pipe if pipe_ends_file(p) => p.bump_error(ParseErrorKind::PipeAtEof),
        SyntaxKind::Pipe if assigned && pipe_ends_statement(p) => {
            p.bump_error(ParseErrorKind::PipeEndsAssignment);
        },
        SyntaxKind::Pipe => {
            p.bump();
            line_end_continuation(p);
        },
        // Nu goes on to the next line only after a plain `|`.
        _ => match element {
            None => p.bump_error(ParseErrorKind::RedirectionAtHead),
            Some(redirected) if redirected.overlaps(Streams::of(kind)) => {
                p.bump_error(ParseErrorKind::MultipleRedirections);
            },
            Some(_) => p.bump(),
        },
    }
}

/// Whether the `|` ahead is the last token of the file, past a trailing
/// comment and comment lines right below it: `Comment? (Newline Comment)*`.
/// A blank line or a final line break saves it; nu counts the `|` as the
/// last token only then.
fn pipe_ends_file(p: &Parser<'_>) -> bool {
    let mut kinds = p
        .remaining_kinds()
        .filter(|&kind| kind != SyntaxKind::Whitespace)
        .skip(1)
        .peekable();
    kinds.next_if_eq(&SyntaxKind::Comment);
    while kinds.next_if_eq(&SyntaxKind::Newline).is_some() {
        if kinds.next() != Some(SyntaxKind::Comment) {
            return false;
        }
    }
    kinds.next().is_none()
}

/// Whether the statement ends at the line break right after the `|` ahead,
/// past a trailing comment.
fn pipe_ends_statement(p: &Parser<'_>) -> bool {
    p.remaining_kinds()
        .filter(|&kind| !matches!(kind, SyntaxKind::Whitespace | SyntaxKind::Comment))
        .nth(1)
        == Some(SyntaxKind::Newline)
        && !next_line_continues(p)
}

/// Whether a `|` starts the next line, past any comment lines.
/// It joins the line before it, along with those comment lines.
fn next_line_starts_with_pipe(p: &Parser<'_>) -> bool {
    line_start(&mut next_lines(p)) == Some(SyntaxKind::Pipe)
}

/// After a `|` at the end of a line, the pipeline goes on at the next line,
/// past any comment lines. A blank line ends it, unless a line-start `|`
/// joins that blank line. `;` ends it too; nu rejects it after `|` anyway,
/// and this keeps the newline out of the pipeline, which must not end with
/// trivia.
fn line_end_continuation(p: &mut Parser<'_>) {
    if p.peek() == Some(SyntaxKind::Newline) && next_line_continues(p) {
        bump_newlines(p);
    }
}

fn next_line_continues(p: &Parser<'_>) -> bool {
    let mut lines = next_lines(p);
    match line_start(&mut lines) {
        Some(SyntaxKind::Newline) => line_start(&mut lines) == Some(SyntaxKind::Pipe),
        Some(kind) => !ends_statement(kind),
        None => false,
    }
}

/// The tokens after the current line, whitespace left out.
fn next_lines(p: &Parser<'_>) -> impl Iterator<Item = SyntaxKind> {
    p.remaining_kinds()
        .filter(|&kind| kind != SyntaxKind::Whitespace)
        .skip_while(|&kind| kind != SyntaxKind::Newline)
        .skip(1)
}

/// The first token of a line, past any comment lines.
/// `Newline` for a blank line, `None` at the end of the file.
fn line_start(lines: &mut impl Iterator<Item = SyntaxKind>) -> Option<SyntaxKind> {
    let mut first = lines.next();
    while first == Some(SyntaxKind::Comment) {
        lines.next();
        first = lines.next();
    }
    first
}

fn bump_newlines(p: &mut Parser<'_>) {
    while p.peek() == Some(SyntaxKind::Newline) {
        p.bump();
    }
}

/// Returns the streams the element redirects; `None` when it has no command.
/// `first` is whether no element comes before it in the pipeline.
fn pipeline_element(p: &mut Parser<'_>, first: bool) -> Option<Streams> {
    p.start(SyntaxKind::PipelineElement);
    let redirected = if assignment_ahead(p) {
        if first && let Some(keyword @ ("let" | "mut" | "const")) = p.peek_text() {
            p.start(SyntaxKind::Let);
            p.bump();
            if let Some((end, error)) = var_type_error(p, keyword) {
                p.bump();
                p.bump_error_to(end, error);
            }
            arguments(p, Some(Streams::default()), false, true);
        } else {
            p.start(SyntaxKind::Assignment);
            command(p, true);
        }
        p.bump();
        assigned_pipeline(p);
        p.finish();
        // No pipe follows what the assignment operator takes.
        None
    } else {
        command(p, false)
    };
    p.finish();
    redirected
}

/// Where the type ahead ends, after a name ending in `:`, and the first error
/// of lexing it as nu does, which reads it up to the assignment operator,
/// leaving out redirections at either end for `const`. `None` also when its
/// tokens hold an error of their own.
fn var_type_error(p: &Parser<'_>, keyword: &str) -> Option<(TextSize, ParseError)> {
    let mut tokens = p
        .remaining_tokens()
        .filter(|(token, _)| !token.kind.is_trivia());
    let (name, text) = tokens.next()?;
    if !text.ends_with(':') {
        return None;
    }
    let mut prev = name.kind;
    let mut range: Option<TextRange> = None;
    for (token, _) in tokens {
        if token.kind == SyntaxKind::Assign && !is_file_redirection(prev) {
            break;
        }
        if token.kind == SyntaxKind::Error {
            return None;
        }
        if keyword != "const" || !(is_file_redirection(token.kind) || is_file_redirection(prev)) {
            range = Some(range.map_or(token.range, |range| range.cover(token.range)));
        }
        prev = token.kind;
    }
    let range = range?;
    let error = p.lex_error(range, Mode::var_type())?;
    Some((range.end(), error.into()))
}

/// Whether an assignment operator comes before the element ends. As in nu,
/// the target of a file redirection is none.
fn assignment_ahead(p: &Parser<'_>) -> bool {
    let mut prev = None;
    for kind in p
        .remaining_kinds()
        .filter(|&kind| !matches!(kind, SyntaxKind::Whitespace | SyntaxKind::Comment))
    {
        if ends_element(kind) {
            return false;
        }
        if kind == SyntaxKind::Assign && !prev.is_some_and(is_file_redirection) {
            return true;
        }
        prev = Some(kind);
    }
    false
}

/// A `Call` or an `ExternalCall`, then its arguments. Returns the streams it
/// redirects; `None` when there is no command.
fn command(p: &mut Parser<'_>, assigning: bool) -> Option<Streams> {
    let has_call = !p
        .peek()
        .is_some_and(|kind| is_file_redirection(kind) || kind == SyntaxKind::Assign);
    let external = p.peek_text().is_some_and(|head| head.starts_with('^'));
    if has_call {
        call(p, external);
    }
    arguments(
        p,
        has_call.then_some(Streams::default()),
        external,
        assigning,
    )
}

/// Items and `Redirection`s up to the end of the element, or up to an
/// assignment operator when `assigning`. `redirected` is what the element
/// redirects so far; `None` when it has no command. Returns what it
/// redirects then.
fn arguments(
    p: &mut Parser<'_>,
    mut redirected: Option<Streams>,
    external: bool,
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
            _ => item(p, external),
        }
    }
    redirected
}

/// What an assignment operator takes: the rest of the statement, where a
/// line-start `|` goes on as after a line-end one.
fn assigned_pipeline(p: &mut Parser<'_>) {
    if p.peek() == Some(SyntaxKind::Newline) && next_line_starts_with_pipe(p) {
        bump_newlines(p);
    }
    if p.peek().is_some_and(|kind| !ends_statement(kind)) {
        pipeline(p, true);
    }
}

fn call(p: &mut Parser<'_>, external: bool) {
    p.start(if external {
        SyntaxKind::ExternalCall
    } else {
        SyntaxKind::Call
    });
    while p.peek().is_some_and(|kind| !ends_call(kind)) {
        item(p, external);
    }
    p.finish();
}

/// `redirected` is what the element redirects so far; `None` when it has
/// no command for the redirection to apply to.
///
/// Nu wants an item right after the operator; anything else is a missing
/// target and stays outside the node.
fn redirection(p: &mut Parser<'_>, kind: SyntaxKind, redirected: Option<&mut Streams>) {
    p.start(SyntaxKind::Redirection);
    let has_target =
        p.nth(1).is_some_and(is_item) && p.nth_text(1).and_then(shell_operator).is_none();
    let streams = Streams::of(kind);
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
        token_or_bracketed(p, false);
    }
    p.finish();
}

fn item(p: &mut Parser<'_>, external: bool) {
    match p.peek_text().and_then(shell_operator) {
        Some(kind) => p.bump_error(kind),
        None => token_or_bracketed(p, external),
    }
}

pub(crate) const SQUARE: Brackets = Brackets {
    open: b'[',
    close: b']',
    open_kind: SyntaxKind::LBracket,
    close_kind: SyntaxKind::RBracket,
};

const CURLY: Brackets = Brackets {
    open: b'{',
    close: b'}',
    open_kind: SyntaxKind::LBrace,
    close_kind: SyntaxKind::RBrace,
};

/// `external` is whether the token is an argument of a `^` command that
/// starts the pipeline element, where nu keeps a word in braces whole.
fn token_or_bracketed(p: &mut Parser<'_>, external: bool) {
    let record = !external
        && p.bracketed(&CURLY, Mode::brace())
            .is_some_and(|bracketed| is_record(bracketed.inner()));
    if let Some(bracketed) = p.bracketed(&SQUARE, Mode::list())
        && is_list(bracketed.inner())
    {
        p.bump_bracketed(SyntaxKind::List, bracketed, list_items);
    } else if record
        && let Some(bracketed) = p.bracketed(&CURLY, Mode::record())
        && split_record(bracketed.inner()).is_some()
    {
        p.bump_bracketed(SyntaxKind::Record, bracketed, record_items);
    } else if !external
        && !record
        && let Some(bracketed) = p.bracketed(&CURLY, Mode::block())
        && is_block(bracketed.inner())
    {
        p.bump_bracketed(SyntaxKind::Block, bracketed, block_items);
    } else {
        p.bump();
    }
}

/// A `;` may make it a table, which stays a word until tables are parsed.
/// A lexing error keeps it a word too, since nothing inside may be reported.
fn is_list(inner: &Parser<'_>) -> bool {
    !inner
        .remaining_kinds()
        .any(|kind| matches!(kind, SyntaxKind::Semicolon | SyntaxKind::Error))
}

/// Unlike `item`, reports no error: nu may read the word as a string.
fn list_items(p: &mut Parser<'_>) {
    while p.peek().is_some() {
        token_or_bracketed(p, false);
    }
}

/// Whether nu reads `{ }` as a record where it takes any value, given its
/// inside in brace mode: a `:` second or a spread first, unless a pipe comes
/// first.
fn is_record(inner: &Parser<'_>) -> bool {
    let mut tokens = inner
        .remaining_tokens()
        .filter(|(token, _)| !token.kind.is_trivia());
    match (tokens.next(), tokens.next()) {
        (Some((first, text)), _) if is_pipe_to_nu(first.kind, text) => false,
        (_, Some((second, _))) if second.kind == SyntaxKind::Colon => true,
        (Some((_, text)), _) => is_spread(text),
        (None, _) => false,
    }
}

/// Whether nu's lexer makes a pipe of the token: `|`, `||`, and what it
/// reports as a mistaken pipe.
fn is_pipe_to_nu(kind: SyntaxKind, text: &str) -> bool {
    matches!(kind, SyntaxKind::Pipe | SyntaxKind::PipePipe)
        || matches!(text, "o>|" | "out>|" | "&&" | "2>&1")
}

/// Whether `text` spreads a record into one, as `...$x`, `...(f)` or
/// `...{a: 1}`.
fn is_spread(text: &str) -> bool {
    text.strip_prefix("...")
        .is_some_and(|rest| rest.starts_with(['$', '(', '{']))
}

/// An item of a record as nu splits it.
enum RecordItem {
    Spread,
    Key,
    /// A key and the token nu takes as its `:`.
    KeyAndColon,
    /// A key, its `:`, and a value of this many touching tokens.
    Pair(usize),
}

/// The items of a record, given its inside in record mode, or `None` when
/// the record stays a word: on a lexing error, or where its tokens cannot
/// make nu's split. nu lexes a key and its `:` as record mode does, but a
/// value differently, so a value is lexed again in its own mode and must
/// cover touching tokens, with no comment among them.
fn split_record(inner: &Parser<'_>) -> Option<Vec<RecordItem>> {
    let tokens: Vec<_> = inner
        .remaining_tokens()
        .filter(|(token, _)| {
            !matches!(
                token.kind,
                SyntaxKind::Whitespace | SyntaxKind::Newline | SyntaxKind::Comma
            )
        })
        .collect();
    if tokens
        .iter()
        .any(|(token, _)| token.kind == SyntaxKind::Error)
    {
        return None;
    }
    let slot = |from: usize| {
        (from..tokens.len()).find(|&index| tokens[index].0.kind != SyntaxKind::Comment)
    };
    let mut items = Vec::new();
    let mut next = 0;
    while let Some(key) = slot(next) {
        if is_spread(tokens[key].1) {
            items.push(RecordItem::Spread);
            next = key + 1;
            continue;
        }
        let Some(colon) = slot(key + 1) else {
            items.push(RecordItem::Key);
            break;
        };
        let Some(start) = slot(colon + 1) else {
            items.push(RecordItem::KeyAndColon);
            break;
        };
        let value = inner.lex_first(tokens[start].0.range.start(), Mode::record_value())?;
        if value.kind == SyntaxKind::Error {
            return None;
        }
        let mut end = start;
        while tokens[end].0.range.end() < value.range.end() {
            end += 1;
            let (token, _) = tokens.get(end)?;
            if token.kind == SyntaxKind::Comment
                || token.range.start() != tokens[end - 1].0.range.end()
            {
                return None;
            }
        }
        if tokens[end].0.range.end() != value.range.end() {
            return None;
        }
        items.push(RecordItem::Pair(end - start + 1));
        next = end + 1;
    }
    Some(items)
}

/// Unlike `item`, reports no error. Keys are strings to nu, so a key and its
/// `:` stay tokens.
fn record_items(p: &mut Parser<'_>) {
    let items = split_record(p).expect("split_record accepted the record");
    for item in items {
        bump_separators(p);
        if let RecordItem::Spread = item {
            p.bump();
            continue;
        }
        p.start(SyntaxKind::RecordField);
        p.bump();
        if matches!(item, RecordItem::KeyAndColon | RecordItem::Pair(_)) {
            bump_separators(p);
            p.bump();
        }
        match item {
            RecordItem::Pair(1) => {
                bump_separators(p);
                token_or_bracketed(p, false);
            },
            RecordItem::Pair(count) => {
                bump_separators(p);
                p.bump_joined(count);
            },
            _ => {},
        }
        p.finish();
    }
    bump_separators(p);
}

fn bump_separators(p: &mut Parser<'_>) {
    while matches!(p.peek(), Some(SyntaxKind::Newline | SyntaxKind::Comma)) {
        p.bump();
    }
}

/// Whether `{ }` that is no record reads as a block or a closure, given its
/// inside in block mode: something but trivia, parameters that close at a
/// `|` with no `;` or shell operator before it, and statements that report
/// no error, since nothing inside may be reported.
fn is_block(inner: &Parser<'_>) -> bool {
    if !inner.remaining_kinds().any(|kind| !kind.is_trivia()) {
        return false;
    }
    if params_start(inner) == Some(SyntaxKind::Pipe) {
        let closes = inner
            .remaining_tokens()
            .skip_while(|(token, _)| token.kind != SyntaxKind::Pipe)
            .skip(1)
            .find(|&(token, text)| {
                matches!(token.kind, SyntaxKind::Semicolon | SyntaxKind::Pipe)
                    || shell_operator(text).is_some()
            })
            .is_some_and(|(token, _)| token.kind == SyntaxKind::Pipe);
        if !closes {
            return false;
        }
    }
    !inner.reports_error(block_items)
}

/// The `|` or `||` that starts parameters where nu finds it: first past
/// whitespace, or a `|` past one line break, which nu's lexer makes that `|`.
fn params_start(p: &Parser<'_>) -> Option<SyntaxKind> {
    let mut kinds = p
        .remaining_kinds()
        .filter(|&kind| kind != SyntaxKind::Whitespace);
    match kinds.next() {
        Some(SyntaxKind::Newline) => kinds.next().filter(|&kind| kind == SyntaxKind::Pipe),
        first => first.filter(|&kind| matches!(kind, SyntaxKind::Pipe | SyntaxKind::PipePipe)),
    }
}

/// Parameters, if any, closing at the next `|`, then statements.
fn block_items(p: &mut Parser<'_>) {
    match params_start(p) {
        Some(SyntaxKind::PipePipe) => {
            p.start(SyntaxKind::ClosureParams);
            p.bump();
            p.finish();
        },
        Some(SyntaxKind::Pipe) => {
            if p.peek() == Some(SyntaxKind::Newline) {
                p.bump();
            }
            p.start(SyntaxKind::ClosureParams);
            p.bump();
            while p.peek() != Some(SyntaxKind::Pipe) {
                p.bump();
            }
            p.bump();
            p.finish();
        },
        _ => {},
    }
    file(p);
}

fn ends_statement(kind: SyntaxKind) -> bool {
    matches!(kind, SyntaxKind::Newline | SyntaxKind::Semicolon)
}

fn ends_element(kind: SyntaxKind) -> bool {
    is_pipe(kind) || ends_statement(kind)
}

fn ends_call(kind: SyntaxKind) -> bool {
    is_file_redirection(kind) || kind == SyntaxKind::Assign || ends_element(kind)
}

fn is_pipe(kind: SyntaxKind) -> bool {
    matches!(
        kind,
        SyntaxKind::Pipe | SyntaxKind::ErrGtPipe | SyntaxKind::OutErrGtPipe
    )
}

/// Stdout and stderr, as what an operator redirects or an element has
/// redirected.
#[derive(Debug, Clone, Copy, Default)]
struct Streams {
    out: bool,
    err: bool,
}

impl Streams {
    fn of(kind: SyntaxKind) -> Streams {
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

    fn overlaps(self, other: Streams) -> bool {
        (self.out && other.out) || (self.err && other.err)
    }

    fn add(&mut self, other: Streams) {
        self.out |= other.out;
        self.err |= other.err;
    }
}

fn is_file_redirection(kind: SyntaxKind) -> bool {
    matches!(
        kind,
        SyntaxKind::OutGt
            | SyntaxKind::OutGtGt
            | SyntaxKind::ErrGt
            | SyntaxKind::ErrGtGt
            | SyntaxKind::OutErrGt
            | SyntaxKind::OutErrGtGt
    )
}

/// Nu rejects `||`, `&&`, `2>`, `2>&1`, and `o>|` wherever they stand alone.
fn shell_operator(text: &str) -> Option<ParseErrorKind> {
    let kind = match text {
        "||" => ParseErrorKind::ShellOrOr,
        "&&" => ParseErrorKind::ShellAndAnd,
        "2>" => ParseErrorKind::ShellErrRedirect,
        "2>&1" => ParseErrorKind::ShellOutErrRedirect,
        "o>|" | "out>|" => ParseErrorKind::OutPipe,
        _ => return None,
    };
    Some(kind)
}

fn is_item(kind: SyntaxKind) -> bool {
    matches!(
        kind,
        SyntaxKind::Word
            | SyntaxKind::String
            | SyntaxKind::RawString
            | SyntaxKind::InterpString
            | SyntaxKind::Error
    )
}

#[cfg(test)]
mod tests {
    use nuten_syntax::SyntaxKind;

    use crate::{ParseErrorKind, parse};

    fn errors(src: &str) -> Vec<ParseErrorKind> {
        parse(src).errors().iter().map(|error| error.kind).collect()
    }

    #[test]
    fn pipe_at_eof_depends_on_a_final_line_break() {
        assert_eq!(errors("ls |"), [ParseErrorKind::PipeAtEof]);
        assert_eq!(errors(";|"), [ParseErrorKind::PipeAtEof]);
        assert_eq!(errors("ls | # c"), [ParseErrorKind::PipeAtEof]);
        assert_eq!(errors("ls |\n# c"), [ParseErrorKind::PipeAtEof]);
        assert_eq!(errors("ls | # c\n# d"), [ParseErrorKind::PipeAtEof]);
        assert_eq!(errors("ls |\n"), []);
        assert_eq!(errors("ls |\n# c\n"), []);
        assert_eq!(errors("ls |\n\n# c"), []);
        assert_eq!(errors("ls e>|"), []);
        assert_eq!(errors("ls | ;"), [ParseErrorKind::SemicolonAfterPipe]);
    }

    #[test]
    fn redirection_target_is_missing_at_eof() {
        assert_eq!(errors("ls o>"), [ParseErrorKind::MissingRedirectionTarget]);
    }

    #[test]
    fn a_redirection_gets_one_error_at_most() {
        assert_eq!(errors("o>"), [ParseErrorKind::RedirectionAtHead]);
        assert_eq!(
            errors("ls o> a o> | x"),
            [ParseErrorKind::MissingRedirectionTarget]
        );
    }

    #[test]
    fn pipe_at_eof_is_reported_at_the_last_pipe() {
        let parsed = parse("ls | | # c");
        assert_eq!(parsed.errors().len(), 1);
        assert_eq!(&"ls | | # c"[parsed.errors()[0].range], "|");
        assert_eq!(parsed.errors()[0].range.start(), 5.into());
    }

    #[test]
    fn brackets_nested_too_deep_stay_a_word() {
        let src = format!("{}{}", "[".repeat(130), "]".repeat(130));
        let tree = parse(&src).syntax();
        let lists = tree
            .descendants()
            .filter(|node| node.kind() == SyntaxKind::List);
        assert_eq!(lists.count(), 128);
        let innermost = tree
            .descendants_with_tokens()
            .filter(|element| element.kind() == SyntaxKind::Word);
        assert_eq!(
            innermost.map(|word| word.to_string()).collect::<Vec<_>>(),
            ["[[]]"]
        );
    }

    #[test]
    fn records_nested_too_deep_stay_a_word() {
        let src = format!("{}1{}", "{a: ".repeat(130), "}".repeat(130));
        let tree = parse(&src).syntax();
        let records = tree
            .descendants()
            .filter(|node| node.kind() == SyntaxKind::Record);
        assert_eq!(records.count(), 128);
        let innermost = tree.descendants_with_tokens().last().unwrap();
        assert_eq!(innermost.to_string(), "}");
        assert!(
            tree.descendants_with_tokens()
                .any(|element| element.kind() == SyntaxKind::Word
                    && element.to_string() == "{a: {a: 1}}")
        );
    }

    #[test]
    fn blocks_nested_too_deep_stay_a_word() {
        let src = format!("{}ls{}", "{".repeat(130), "}".repeat(130));
        let tree = parse(&src).syntax();
        let blocks = tree
            .descendants()
            .filter(|node| node.kind() == SyntaxKind::Block);
        assert_eq!(blocks.count(), 128);
        assert!(
            tree.descendants_with_tokens().any(
                |element| element.kind() == SyntaxKind::Word && element.to_string() == "{{ls}}"
            )
        );
    }
}
