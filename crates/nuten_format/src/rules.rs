//! Turns each node of the tree into document elements.

use std::{iter, mem};

use nuten_doc::{
    Element, empty_line, empty_lines, expand_parent, group, hard_line_break, indent, line_suffix,
    soft_line_break, soft_line_break_or_space, space, text,
};
use nuten_parser::lexer::{self, Mode};
use nuten_syntax::{SyntaxElement, SyntaxKind, SyntaxNode, SyntaxToken};
use rowan::TextRange;

/// A child of a container: neither whitespace nor a line break.
struct Child {
    element: SyntaxElement,
    /// Line breaks right before it; two or more make a blank line.
    newlines: u32,
}

/// The children of `parent` in source order. Line breaks at its end, which
/// only Root has, belong to no child, so nothing is printed for them.
fn children(parent: &SyntaxNode) -> impl Iterator<Item = Child> {
    let mut newlines = 0;
    parent
        .children_with_tokens()
        .filter_map(move |element| match element.kind() {
            SyntaxKind::Whitespace => None,
            SyntaxKind::Newline => {
                newlines += 1;
                None
            },
            _ => {
                let child = Child {
                    element,
                    newlines,
                };
                newlines = 0;
                Some(child)
            },
        })
}

/// `root` is the tree of `src`, with no `Error` element.
pub(crate) fn file<'a>(src: &'a str, root: &SyntaxNode) -> Vec<Element<'a>> {
    let mut out = statements(src, children(root), true);
    out.push(hard_line_break());
    out
}

/// Statements; line breaks before the first are left to the container.
/// Blank lines stay, one at most, or two after a statement ending in `|`.
/// A comment stays on its own line or at the end of its line; `at_start` is
/// whether nothing comes before the first child on its line.
fn statements(
    src: &str,
    items: impl IntoIterator<Item = Child>,
    at_start: bool,
) -> Vec<Element<'_>> {
    let mut out = Vec::new();
    let mut prev = None;
    // Whether a `|` at the end of the last statement still reaches what
    // comes next.
    let mut open_pipe = false;
    for item in items {
        let Child {
            element,
            newlines,
        } = item;
        let kind = element.kind();
        if newlines > 0 && prev.is_some() {
            out.push(match newlines {
                1 => hard_line_break(),
                // With one blank line, a `|` at the start of the next line
                // would join the statement above, so two stay.
                3.. if open_pipe => empty_lines(2),
                _ => empty_line(),
            });
            // A blank line closes the pipe above, as it did in the source.
            if newlines > 1 {
                open_pipe = false;
            }
        }
        match &element {
            SyntaxElement::Node(child) => {
                if prev == Some(SyntaxKind::Semicolon) && newlines == 0 {
                    out.push(space());
                }
                out.extend(node(src, child));
                open_pipe = ends_with_pipe(child);
            },
            SyntaxElement::Token(child) => match kind {
                SyntaxKind::Semicolon => out.push(token(src, child)),
                SyntaxKind::Comment => {
                    out.extend(comment(src, child, newlines, at_start && prev.is_none()));
                },
                kind => unreachable!("{kind:?} is not among statements"),
            },
        }
        prev = Some(kind);
    }
    out
}

/// Whether the statement ends with a `|`; statements never end with
/// trivia, so the last child tells.
fn ends_with_pipe(statement: &SyntaxNode) -> bool {
    statement
        .last_child_or_token()
        .is_some_and(|last| last.kind() == SyntaxKind::Pipe)
}

/// The entry for every node but Root.
fn node<'a>(src: &'a str, node: &SyntaxNode) -> Vec<Element<'a>> {
    match node.kind() {
        SyntaxKind::Pipeline => pipeline(src, node),
        SyntaxKind::PipelineElement
        | SyntaxKind::Call
        | SyntaxKind::ExternalCall
        | SyntaxKind::Redirection => spaced(src, node),
        SyntaxKind::List | SyntaxKind::Record if reads_as_value(node) => container(src, node),
        SyntaxKind::List | SyntaxKind::Record => vec![text(&src[node.text_range()])],
        SyntaxKind::Block if in_match(node) => vec![text(&src[node.text_range()])],
        SyntaxKind::Block => block(src, node),
        kind => unreachable!("{kind:?} is not formatted as a node"),
    }
}

/// On one line when no line break came before a pipe or element in the
/// source and it fits, counting only the first line of an element that ends
/// it; otherwise each `|` starts a line, at the current indent, while `e>|`
/// and `o+e>|` stay on the line of their element.
/// A comment stays where it is among the pipes and elements, on its own
/// line or at the end of its line. A pipe with a comment after it stays at
/// the end of its line, and what comes next starts a line.
fn pipeline<'a>(src: &'a str, pipeline: &SyntaxNode) -> Vec<Element<'a>> {
    let mut out = Vec::new();
    let mut prev = None;
    // Where an element that ends the pipeline, with its separator, starts in
    // `out`.
    let mut tail = None;
    for part in parts(pipeline) {
        let Part {
            child: Child {
                element,
                newlines,
            },
            joined,
        } = part;
        let kind = element.kind();
        if newlines > 0 {
            out.push(expand_parent());
        }
        if joined {
            out.push(space());
            match &element {
                SyntaxElement::Node(child) => out.extend(node(src, child)),
                SyntaxElement::Token(child) => out.push(token(src, child)),
            }
            prev = Some(kind);
            continue;
        }
        tail = None;
        match &element {
            SyntaxElement::Node(child) => {
                tail = Some(out.len());
                out.extend(separator(prev, space));
                out.extend(node(src, child));
            },
            // `let`, or an assignment operator with nothing before it, which
            // starts an element spread out.
            SyntaxElement::Token(child)
                if matches!(kind, SyntaxKind::Word | SyntaxKind::Assign) =>
            {
                tail = Some(out.len());
                out.extend(separator(prev, space));
                out.push(token(src, child));
            },
            SyntaxElement::Token(child) => match kind {
                SyntaxKind::Pipe => {
                    out.extend(separator(prev, || {
                        if comment_follows(child) {
                            space()
                        } else {
                            soft_line_break_or_space()
                        }
                    }));
                    out.push(token(src, child));
                },
                // Nu rejects these at the start of a line, so they stay
                // after their element, which the parser puts right before.
                SyntaxKind::ErrGtPipe | SyntaxKind::OutErrGtPipe => {
                    out.extend(separator(prev, space));
                    out.push(token(src, child));
                },
                SyntaxKind::Comment => out.extend(comment(src, child, newlines, prev.is_none())),
                kind => unreachable!("{kind:?} is not a child of Pipeline"),
            },
        }
        prev = Some(kind);
    }
    // The tail is measured in break mode, so its first line break ends the
    // measure instead of breaking the group.
    let tail = out.split_off(tail.unwrap_or(out.len()));
    if out.is_empty() {
        return tail;
    }
    iter::once(group(out)).chain(tail).collect()
}

/// Whether nu reads `node`, a list or a record, as one whatever the command:
/// as the head of a command, as an element of a list, or as the value of a
/// field. Elsewhere, such as an argument, a command may read it as a string.
fn reads_as_value(node: &SyntaxNode) -> bool {
    node.parent().is_some_and(|parent| match parent.kind() {
        // Only a value of a field is a node.
        SyntaxKind::List | SyntaxKind::RecordField => true,
        SyntaxKind::Call => {
            parent
                .first_child_or_token()
                .and_then(SyntaxElement::into_node)
                == Some(node.clone())
        },
        _ => false,
    })
}

/// A list or a record, unless it stays as it is. On one line when it fits,
/// no line break comes right after the opening bracket, and no comment is
/// inside, with no space inside the brackets of a list and one inside those
/// of a record; otherwise one item per line, indented, with the closing
/// bracket on a line of its own. A `,` is dropped. A blank line between items
/// stays.
fn container<'a>(src: &'a str, container: &SyntaxNode) -> Vec<Element<'a>> {
    if stays_as_is(src, container) {
        return vec![text(&src[container.text_range()])];
    }
    let (Some(open), Some(close)) = (container.first_token(), container.last_token()) else {
        unreachable!("a container has brackets");
    };
    let padding = if container.kind() == SyntaxKind::Record {
        soft_line_break_or_space
    } else {
        soft_line_break
    };
    let mut children = children(container).skip(1).peekable();
    // A line break right after the opening bracket.
    let mut expand = children.peek().is_some_and(|child| child.newlines > 0);
    let mut inner = Vec::new();
    let mut prev = None;
    // The most line breaks before a `,` dropped since the last item, which
    // count as line breaks before what follows.
    let mut dropped = 0;
    for child in children {
        let Child {
            element,
            newlines,
        } = child;
        let newlines = newlines.max(mem::take(&mut dropped));
        let kind = element.kind();
        let separator = || match prev {
            None => padding(),
            Some(_) if newlines > 1 => empty_line(),
            // After a comment, the container is expanded, so the line break
            // comes anyway.
            Some(_) => soft_line_break_or_space(),
        };
        match &element {
            SyntaxElement::Node(child) if kind == SyntaxKind::RecordField => {
                inner.push(separator());
                inner.extend(field(src, child));
            },
            SyntaxElement::Node(child) => {
                inner.push(separator());
                inner.extend(node(src, child));
            },
            SyntaxElement::Token(_)
                if matches!(kind, SyntaxKind::RBracket | SyntaxKind::RBrace) =>
            {
                break;
            },
            SyntaxElement::Token(_) if kind == SyntaxKind::Comma => {
                dropped = newlines;
                continue;
            },
            SyntaxElement::Token(child) if kind == SyntaxKind::Comment => {
                expand = true;
                if prev.is_some() && newlines > 1 {
                    inner.push(empty_line());
                }
                inner.extend(comment(src, child, newlines, false));
            },
            SyntaxElement::Token(child) => {
                inner.push(separator());
                inner.push(token(src, child));
            },
        }
        prev = Some(kind);
    }
    if inner.is_empty() {
        return vec![token(src, &open), token(src, &close)];
    }
    let mut out = vec![
        token(src, &open),
        indent(inner),
        padding(),
        token(src, &close),
    ];
    if expand {
        out.push(expand_parent());
    }
    vec![group(out)]
}

/// Whether `block` is an argument of `match`, its subject or its arms, which
/// stay as they are.
fn in_match(block: &SyntaxNode) -> bool {
    block.parent().is_some_and(|parent| {
        parent.kind() == SyntaxKind::Call
            && parent
                .first_child_or_token()
                .and_then(SyntaxElement::into_token)
                .is_some_and(|head| head.kind() == SyntaxKind::Word && head.text() == "match")
    })
}

/// A block or a closure. On one line when it fits and no line break comes
/// after the parameters, with one space inside the braces; otherwise the
/// statements go indented, with the closing brace on a line of its own.
/// Blank lines at the start and the end go. The parameters stay as they are,
/// along with what comes before them.
fn block<'a>(src: &'a str, block: &SyntaxNode) -> Vec<Element<'a>> {
    let mut items = children(block).peekable();
    let Some(open) = items.next() else {
        unreachable!("a block has braces");
    };
    let params = items.next_if(|item| item.element.kind() == SyntaxKind::ClosureParams);
    let head_end = params
        .as_ref()
        .map_or(open.element.text_range(), |params| {
            params.element.text_range()
        })
        .end();
    let head = text(&src[TextRange::new(open.element.text_range().start(), head_end)]);
    let mut body: Vec<_> = items.collect();
    let Some(Child {
        element: SyntaxElement::Token(close),
        newlines: close_newlines,
    }) = body.pop()
    else {
        unreachable!("a block has braces");
    };
    let Some(first) = body.first() else {
        return vec![head, token(src, &close)];
    };
    // A comment is followed by a line break, so it expands the block too.
    let expand = close_newlines > 0 || body.iter().any(|item| item.newlines > 0);
    let lead = if first.element.kind() == SyntaxKind::Comment && first.newlines == 0 {
        None
    } else if params.is_none()
        && first
            .element
            .as_node()
            .and_then(SyntaxNode::first_child_or_token)
            .is_some_and(|first| first.kind() == SyntaxKind::Pipe)
    {
        // Right after the brace and a line break, nu would read the `|` as
        // the start of parameters.
        Some(empty_line())
    } else {
        Some(soft_line_break_or_space())
    };
    let inner = lead.into_iter().chain(statements(src, body, false));
    let mut out = vec![
        head,
        indent(inner),
        soft_line_break_or_space(),
        token(src, &close),
    ];
    if expand {
        out.push(expand_parent());
    }
    vec![group(out)]
}

/// Whether `container` stays as it is: a comment inside keeps it so, or, for
/// a record, dropping a `,` would change the first two tokens nu looks at to
/// tell a record from a closure, which it lexes with `,` as no whitespace.
fn stays_as_is(src: &str, container: &SyntaxNode) -> bool {
    let kept_by_comment = container
        .children_with_tokens()
        .flat_map(|child| {
            let field = child
                .as_node()
                .filter(|node| node.kind() == SyntaxKind::RecordField)
                .map(SyntaxNode::children_with_tokens);
            let own = field.is_none().then_some(child);
            own.into_iter().chain(field.into_iter().flatten())
        })
        .filter_map(SyntaxElement::into_token)
        .any(|token| token.kind() == SyntaxKind::Comment && keeps_as_is(&token));
    kept_by_comment || container.kind() == SyntaxKind::Record && !keeps_first_tokens(src, container)
}

/// Whether the first two tokens of the inside of `record`, lexed as nu does to
/// tell a record from a closure, stay the same once the commas are dropped.
/// The output has no `,` and `:` is special in both lexings, so its tokens
/// are those of `record`. A `,` ending a token does not count, as nu looks
/// only at how the first token starts and whether the second is `:`.
fn keeps_first_tokens(src: &str, record: &SyntaxNode) -> bool {
    let (Some(open), Some(close)) = (record.first_token(), record.last_token()) else {
        unreachable!("a record has braces");
    };
    let inside = TextRange::new(open.text_range().end(), close.text_range().start());
    let lexed = lexer::lex(src, inside, Mode::brace());
    let source = lexed
        .tokens
        .iter()
        .filter(|token| !token.kind.is_trivia())
        .map(|token| src[token.range].trim_end_matches(',').to_owned());
    let output = record
        .descendants_with_tokens()
        .filter_map(SyntaxElement::into_token)
        .filter(|token| {
            !token.kind().is_trivia()
                && !matches!(
                    token.kind(),
                    SyntaxKind::LBrace | SyntaxKind::RBrace | SyntaxKind::Comma
                )
        })
        .map(|token| token.text().to_owned());
    source.take(2).eq(output.take(2))
}

/// A key, its `:` right after it, and its value one space after that. What
/// follows a comment inside starts a line. A `,` is dropped.
fn field<'a>(src: &'a str, field: &SyntaxNode) -> Vec<Element<'a>> {
    let mut out = Vec::new();
    let mut prev = None;
    // Parts printed, comments and commas aside; the key is the first.
    let mut parts = 0;
    for child in children(field) {
        let Child {
            element,
            newlines,
        } = child;
        let kind = element.kind();
        match &element {
            SyntaxElement::Token(_) if kind == SyntaxKind::Comma => continue,
            SyntaxElement::Token(child) if kind == SyntaxKind::Comment => {
                out.extend(comment(src, child, newlines, false));
            },
            _ => {
                match prev {
                    None => {},
                    Some(SyntaxKind::Comment) => out.push(hard_line_break()),
                    Some(_) if parts == 1 && kind == SyntaxKind::Colon => {},
                    Some(_) => out.push(space()),
                }
                match &element {
                    SyntaxElement::Node(child) => out.extend(node(src, child)),
                    SyntaxElement::Token(child) => out.push(token(src, child)),
                }
                parts += 1;
            },
        }
        prev = Some(kind);
    }
    out
}

/// Whether `comment` makes the list or record it is in stay as it is. Right
/// after anything but whitespace, nu does not start a comment when it reads
/// the word the list or record is in, so a space put before it may move where
/// the word ends. A lone `\r` at its end would become part of a line break.
fn keeps_as_is(comment: &SyntaxToken) -> bool {
    comment.text().ends_with('\r')
        || comment
            .prev_sibling_or_token()
            .is_some_and(|prev| !prev.kind().is_trivia())
}

/// A child of a pipeline as `pipeline` formats it.
struct Part {
    child: Child,
    /// Whether it belongs to the element before it, one space after.
    joined: bool,
}

/// The children of `pipeline`, with each element that holds an `Assignment`
/// or a `Let` spread out: its parts up to the operator, then any comment,
/// then the children of the pipeline it takes, whose first element joins
/// the operator unless a line break or a comment comes between. So the
/// pipes of what the operator takes format as those of `pipeline`.
fn parts(pipeline: &SyntaxNode) -> Vec<Part> {
    let mut out = Vec::new();
    spread(pipeline, 0, false, &mut out);
    out
}

/// `newlines` come before the first child, and `joined` is whether it joins
/// what comes before when it is an element.
fn spread(pipeline: &SyntaxNode, newlines: u32, joined: bool, out: &mut Vec<Part>) {
    let mut first = Some((newlines, joined));
    for child in children(pipeline) {
        let (before, joined) = first.take().unwrap_or((0, false));
        let newlines = before + child.newlines;
        let assignment = child
            .element
            .as_node()
            .and_then(SyntaxNode::first_child)
            .filter(|node| matches!(node.kind(), SyntaxKind::Assignment | SyntaxKind::Let));
        match assignment {
            Some(assignment) => spread_assignment(&assignment, newlines, joined, out),
            None => out.push(Part {
                joined: joined && child.element.as_node().is_some(),
                child: Child {
                    element: child.element,
                    newlines,
                },
            }),
        }
    }
}

/// The parts of `assignment` for `spread`: each joins the part before it but
/// a comment, and the first, which joins as `joined` tells.
fn spread_assignment(assignment: &SyntaxNode, newlines: u32, joined: bool, out: &mut Vec<Part>) {
    let mut first = Some((newlines, joined));
    for child in children(assignment) {
        let (before, joined) = first.take().unwrap_or((0, true));
        let Child {
            element,
            newlines,
        } = child;
        let newlines = before + newlines;
        let kind = element.kind();
        match &element {
            SyntaxElement::Node(pipeline) if kind == SyntaxKind::Pipeline => {
                spread(pipeline, newlines, true, out);
            },
            _ => out.push(Part {
                joined: joined && kind != SyntaxKind::Comment,
                child: Child {
                    element,
                    newlines,
                },
            }),
        }
    }
}

/// What separates the child after `prev` from it. A comment runs to the end
/// of its line, so a line break follows one.
fn separator(
    prev: Option<SyntaxKind>,
    plain: impl FnOnce() -> Element<'static>,
) -> Option<Element<'static>> {
    match prev {
        None => None,
        Some(SyntaxKind::Comment) => Some(hard_line_break()),
        Some(_) => Some(plain()),
    }
}

/// Whether a comment comes before the next pipe or element after `pipe`.
fn comment_follows(pipe: &SyntaxToken) -> bool {
    let mut next = pipe.next_sibling_or_token();
    while let Some(sibling) = &next {
        match sibling.kind() {
            SyntaxKind::Comment => return true,
            SyntaxKind::Whitespace | SyntaxKind::Newline => next = sibling.next_sibling_or_token(),
            _ => return false,
        }
    }
    false
}

/// The children with one space between each two. No line break falls among
/// them and no node ends with trivia, so a comment, which runs to the end of
/// its line, cannot be here either.
fn spaced<'a>(src: &'a str, parent: &SyntaxNode) -> Vec<Element<'a>> {
    let mut out = Vec::new();
    for child in children(parent) {
        debug_assert!(
            child.newlines == 0 && child.element.kind() != SyntaxKind::Comment,
            "{:?} holds a line break or a comment",
            parent.kind()
        );
        if !out.is_empty() {
            out.push(space());
        }
        match &child.element {
            SyntaxElement::Node(child) => out.extend(node(src, child)),
            SyntaxElement::Token(child) => out.push(token(src, child)),
        }
    }
    out
}

/// The one place a comment is printed, other than inside text kept as it is.
/// `at_start` is whether nothing comes before it on its line.
fn comment<'a>(
    src: &'a str,
    comment: &SyntaxToken,
    newlines: u32,
    at_start: bool,
) -> Vec<Element<'a>> {
    if newlines > 0 || at_start {
        // The container may have pushed a line break already; the printer
        // keeps the longer of the two.
        vec![hard_line_break(), token(src, comment)]
    } else {
        // A line suffix, so that it does not count toward the width of the
        // line.
        vec![line_suffix([space(), token(src, comment)])]
    }
}

/// As it is in the source.
fn token<'a>(src: &'a str, token: &SyntaxToken) -> Element<'a> {
    text(&src[token.text_range()])
}
