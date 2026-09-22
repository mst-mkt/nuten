use nuten_doc::{
    Element, GroupId, align, align_if_group_breaks, expand_parent, group_with_id,
    soft_line_break_or_space, space,
};
use nuten_syntax::{
    NodeOrToken, SyntaxKind, SyntaxNode,
    ast::{AstNode, Node, Pipeline},
};

use super::{Child, Formatter, children, separator};

impl<'a> Formatter<'a> {
    pub(super) fn pipeline(&mut self, pipeline: &Pipeline) -> Vec<Element<'a>> {
        let id = self.groups.fresh();
        let parts = parts(pipeline.syntax());
        let expand = parts.iter().any(|part| part.child.newlines > 0);
        let (segments, tail) = segments(&parts, follows_semicolon(pipeline.syntax()));
        let head: Vec<_> = expand
            .then(expand_parent)
            .into_iter()
            .chain(
                segments
                    .iter()
                    .flat_map(|segment| self.segment(segment, id)),
            )
            .collect();
        // The tail is measured in break mode.
        // Its first line break then ends the measure instead of breaking the group.
        let tail = tail.map_or_else(Vec::new, |tail| self.segment(&tail, id));
        let assigned = self.assigned(pipeline.syntax());
        let group = (!head.is_empty()).then(|| group_with_id(id, head));
        group.into_iter().chain(tail).chain(assigned).collect()
    }

    fn assigned(&mut self, pipeline: &SyntaxNode) -> Vec<Element<'a>> {
        let assigned = assignment_of(pipeline).map(|assignment| assigned_children(&assignment));
        let Some((items, Some(first))) = assigned else {
            return Vec::new();
        };
        items
            .iter()
            .enumerate()
            .skip(first + 1)
            .flat_map(|(index, item)| self.statement(&items[..index], item, false))
            .collect()
    }

    fn segment(&mut self, segment: &Segment<'_>, id: GroupId) -> Vec<Element<'a>> {
        let pipe = segment.pipe.map(|pipe| {
            // A pipe with a comment after it stays at the end of its line.
            let plain = if segment.starts_with_comment() {
                space()
            } else {
                soft_line_break_or_space()
            };
            separator(segment.prev, Some(plain))
                .into_iter()
                .chain(self.element(&pipe.child.element))
        });
        let end = segment.aligned_end();
        let aligned = segment.items[..end]
            .iter()
            .enumerate()
            .flat_map(|(index, part)| self.part(segment.prev_of(index), part))
            .collect();
        let aligned = match segment.align {
            Some(Alignment::Always) => vec![align(2, aligned)],
            Some(Alignment::IfGroupBreaks) => vec![align_if_group_breaks(id, 2, aligned)],
            None => aligned,
        };
        let rest = segment.items[end..]
            .iter()
            .enumerate()
            .flat_map(|(index, part)| self.part(segment.prev_of(end + index), part));
        pipe.into_iter()
            .flatten()
            .chain(aligned)
            .chain(rest)
            .collect()
    }

    fn part(&mut self, prev: Option<SyntaxKind>, part: &Part) -> Vec<Element<'a>> {
        let Child {
            element,
            newlines,
        } = &part.child;
        match element {
            NodeOrToken::Token(comment) if comment.kind() == SyntaxKind::Comment => {
                self.comment(comment, *newlines, prev.is_none())
            },
            // Nu rejects `e>|` and `o+e>|` starting a line, so they stay after their element.
            // The parser puts that element right before them.
            _ if part.joined || part.is_element() || part.is_redirection_pipe() => {
                separator(prev, Some(space()))
                    .into_iter()
                    .chain(self.element(element))
                    .collect()
            },
            _ => unreachable!("{:?} is not a child of Pipeline", part.kind()),
        }
    }
}

fn follows_semicolon(node: &SyntaxNode) -> bool {
    let mut before = node.prev_sibling_or_token();
    while let Some(sibling) = &before
        && sibling.kind() == SyntaxKind::Whitespace
    {
        before = sibling.prev_sibling_or_token();
    }
    before.is_some_and(|before| before.kind() == SyntaxKind::Semicolon)
}

/// A child of a pipeline as `pipeline` formats it.
struct Part {
    child: Child,
    /// Whether it belongs to the element before it, so the pipeline does not end at it.
    joined: bool,
}

impl Part {
    fn kind(&self) -> SyntaxKind {
        self.child.kind()
    }

    /// Whether it is an element of the pipeline, or what starts one of a flattened assignment.
    /// A `let`, the head, and the operator each start one.
    /// The head is a bare item where nu reads an expression instead of a call.
    /// So they are what the pipeline may end with.
    fn is_element(&self) -> bool {
        match &self.child.element {
            NodeOrToken::Node(_) => true,
            NodeOrToken::Token(token) => {
                let kind = token.kind();
                kind.is_item() || kind == SyntaxKind::Assign
            },
        }
    }

    fn is_redirection_pipe(&self) -> bool {
        matches!(
            self.kind(),
            SyntaxKind::ErrGtPipe | SyntaxKind::OutErrGtPipe
        )
    }
}

/// An `Assignment` or a `Let` is laid flat.
/// The pipes of the first statement the operator takes then format as those of `pipeline`.
/// The statements below it are left to `assigned`.
fn parts(pipeline: &SyntaxNode) -> Vec<Part> {
    flatten(pipeline, 0, false)
}

fn assignment_of(pipeline: &SyntaxNode) -> Option<SyntaxNode> {
    pipeline
        .children()
        .filter_map(|element| element.first_child())
        .find(is_assignment)
}

fn is_assignment(node: &SyntaxNode) -> bool {
    matches!(node.kind(), SyntaxKind::Assignment | SyntaxKind::Let)
}

fn assigned_children(assignment: &SyntaxNode) -> (Vec<Child>, Option<usize>) {
    let items: Vec<_> = children(assignment.children_with_tokens()).collect();
    let first = items.iter().position(Child::is_statement);
    (items, first)
}

fn flatten(pipeline: &SyntaxNode, newlines: u32, joined: bool) -> Vec<Part> {
    children(pipeline.children_with_tokens())
        .enumerate()
        .flat_map(|(index, child)| {
            let (before, joined) = if index == 0 {
                (newlines, joined)
            } else {
                (0, false)
            };
            let child = Child {
                element: child.element,
                newlines: before + child.newlines,
            };
            let assignment = child
                .element
                .as_node()
                .and_then(|node| node.syntax().first_child())
                .filter(is_assignment);
            match assignment {
                Some(assignment) => flatten_assignment(&assignment, child.newlines, joined),
                None => vec![Part {
                    joined: joined && child.element.as_node().is_some(),
                    child,
                }],
            }
        })
        .collect()
}

fn flatten_assignment(assignment: &SyntaxNode, newlines: u32, joined: bool) -> Vec<Part> {
    let (items, first) = assigned_children(assignment);
    let end = first.map_or(items.len(), |first| first + 1);
    items
        .into_iter()
        .take(end)
        .enumerate()
        .flat_map(|(index, child)| {
            let (before, joined) = if index == 0 {
                (newlines, joined)
            } else {
                (0, true)
            };
            let child = Child {
                element: child.element,
                newlines: before + child.newlines,
            };
            match &child.element {
                NodeOrToken::Node(Node::Pipeline(pipeline)) => {
                    flatten(pipeline.syntax(), child.newlines, true)
                },
                _ => vec![Part {
                    joined: joined && !child.is_comment(),
                    child,
                }],
            }
        })
        .collect()
}

/// How what follows a `|` is aligned.
#[derive(Clone, Copy)]
enum Alignment {
    /// Always, since the `|` starts a line even when the pipeline is on one line.
    Always,
    IfGroupBreaks,
}

/// A `|` and what follows it, up to the next `|`.
/// The first segment of a pipeline has no `|`, and neither has the tail.
/// The tail is the element that ends the pipeline, once `split_tail` takes it off the last segment.
#[derive(Clone, Copy)]
struct Segment<'p> {
    prev: Option<SyntaxKind>,
    pipe: Option<&'p Part>,
    items: &'p [Part],
    align: Option<Alignment>,
}

fn starts_with_comment(items: &[Part]) -> bool {
    items.first().is_some_and(|part| part.child.is_comment())
}

fn segments(parts: &[Part], after_semicolon: bool) -> (Vec<Segment<'_>>, Option<Segment<'_>>) {
    let is_pipe = |part: &Part| part.kind() == SyntaxKind::Pipe;
    let mut runs = parts.split(is_pipe);
    let first = Segment {
        prev: None,
        pipe: None,
        items: runs.next().expect("split yields a run before any pipe"),
        align: None,
    };
    let (mut segments, last) = parts.iter().filter(|part| is_pipe(part)).zip(runs).fold(
        (Vec::new(), first),
        |(mut segments, last), (pipe, items)| {
            let segment = Segment::of_pipe(last.last_kind(), pipe, items, after_semicolon);
            segments.push(last);
            (segments, segment)
        },
    );
    let (last, tail) = last.split_tail();
    segments.push(last);
    (segments, tail)
}

impl<'p> Segment<'p> {
    fn of_pipe(
        prev: Option<SyntaxKind>,
        pipe: &'p Part,
        items: &'p [Part],
        after_semicolon: bool,
    ) -> Self {
        // A statement starts a line in the output, but not right after a `;` on the same line.
        // Its `|` never starts a line there.
        // In a block on one line, nothing breaks anyway.
        let align = if starts_with_comment(items) || (prev.is_none() && after_semicolon) {
            None
        } else if matches!(prev, None | Some(SyntaxKind::Comment)) {
            Some(Alignment::Always)
        } else {
            Some(Alignment::IfGroupBreaks)
        };
        Segment {
            prev,
            pipe: Some(pipe),
            items,
            align,
        }
    }

    fn starts_with_comment(&self) -> bool {
        starts_with_comment(self.items)
    }

    fn prev_of(&self, index: usize) -> Option<SyntaxKind> {
        self.items[..index]
            .last()
            .map(Part::kind)
            .or(self.pipe.map(Part::kind))
            .or(self.prev)
    }

    fn last_kind(&self) -> Option<SyntaxKind> {
        self.prev_of(self.items.len())
    }

    fn aligned_end(&self) -> usize {
        self.items
            .iter()
            .position(|part| part.child.is_comment())
            .unwrap_or(self.items.len())
    }

    fn split_tail(&self) -> (Self, Option<Self>) {
        let last = self.items.iter().rposition(|part| !part.joined);
        let Some(tail) = last.filter(|&last| self.items[last].is_element()) else {
            return (*self, None);
        };
        let (items, tail_items) = self.items.split_at(tail);
        let head = Segment {
            items,
            ..*self
        };
        let tail = Segment {
            prev: self.prev_of(tail),
            pipe: None,
            items: tail_items,
            align: self
                .align
                .filter(|_| !items.iter().any(|part| part.child.is_comment())),
        };
        (head, Some(tail))
    }
}
