use std::iter;

use nuten_doc::{
    Element, GroupId, align, align_if_group_breaks, empty_line, expand_parent, group_with_id,
    hard_line_break, indent_if_group_breaks, soft_line_break_or_space, space,
};
use nuten_syntax::{
    NodeOrToken, SyntaxKind, SyntaxNode,
    ast::{self, AstNode, Node, Pipeline},
};

use super::{Child, Formatter, children, headed_by_attribute, kept::Kept, separator};

impl<'a> Formatter<'a> {
    pub(super) fn pipeline(&mut self, pipeline: &Pipeline) -> Vec<Element<'a>> {
        let id = self.groups.fresh();
        let parts = parts(pipeline.syntax(), &self.kept);
        let expand = parts.iter().any(|part| part.child.newlines > 0);
        let (segments, tail) = segments(&parts, follows_semicolon(pipeline.syntax()), |part| {
            part.child
                .element
                .as_node()
                .is_some_and(|node| self.breaks_at_operators(node.syntax()))
        });
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
        let assigned = assignment_of(pipeline, &self.kept)
            .map(|assignment| assigned_children(&assignment, &self.kept));
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
            let gap = if pipe.keeps_blank_line_after_attribute() {
                // Not through `separator`, which leaves only a line break after a comment.
                Some(empty_line())
            } else {
                // A pipe with a comment after it stays at the end of its line.
                // So does one before an attribute, which nu reads as one only after a line break.
                let plain = if segment.keeps_pipe_at_line_end() {
                    space()
                } else {
                    soft_line_break_or_space()
                };
                separator(segment.prev, Some(plain))
            };
            gap.into_iter().chain(self.element(&pipe.child.element))
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
        let leading = pipe.into_iter().flatten().chain(aligned).collect();
        // From the first comment on, each part may start a line of its own.
        // So each takes its own indent, rather than that of the segment.
        let rest = segment.items[end..]
            .iter()
            .enumerate()
            .flat_map(|(index, part)| {
                let elements = self.part(segment.prev_of(end + index), part).collect();
                indent_levels_if_group_breaks(id, part.level, elements)
            });
        let level = segment
            .pipe
            .or(segment.items.first())
            .map_or(0, |part| part.level);
        indent_levels_if_group_breaks(id, level, leading)
            .into_iter()
            .chain(rest)
            .collect()
    }

    fn part(
        &mut self,
        prev: Option<SyntaxKind>,
        part: &Part,
    ) -> impl Iterator<Item = Element<'a>> + use<'a> {
        let Child {
            element,
            newlines,
        } = &part.child;
        let (lead, elements) = match element {
            NodeOrToken::Token(comment) if comment.kind() == SyntaxKind::Comment => (
                part.keeps_blank_line_after_attribute().then(empty_line),
                self.comment(comment, *newlines, prev.is_none()),
            ),
            // Nu rejects `e>|` and `o+e>|` starting a line, except right after an attribute.
            // So they stay after their element, which the parser puts right before them.
            // After an attribute, one on the same line would be in the attribute instead.
            // An element that an attribute heads keeps its line, since nu reads one only there.
            _ if part.is_element() || part.is_redirection_pipe() => {
                let plain = if part.starts_line_with_attribute()
                    || (part.is_redirection_pipe() && *newlines > 0)
                {
                    hard_line_break()
                } else {
                    space()
                };
                (separator(prev, Some(plain)), self.element(element))
            },
            _ => unreachable!("{:?} is not a child of Pipeline", part.kind()),
        };
        lead.into_iter().chain(elements)
    }
}

fn indent_levels_if_group_breaks(
    id: GroupId,
    level: u32,
    elements: Vec<Element<'_>>,
) -> Vec<Element<'_>> {
    (0..level).fold(elements, |elements, _| {
        vec![indent_if_group_breaks(id, elements)]
    })
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
    /// Its indent level when it starts a line.
    level: u32,
}

impl Part {
    fn kind(&self) -> SyntaxKind {
        self.child.kind()
    }

    /// Whether it is an element of the pipeline, or what starts one of a flattened assignment.
    /// A `let`, the head, and the operator each start one.
    /// The head is a bare item where nu reads a value with no operator after it.
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

    fn starts_line_with_attribute(&self) -> bool {
        self.child.newlines > 0
            && self
                .child
                .element
                .as_node()
                .is_some_and(|node| headed_by_attribute(node.syntax()))
    }

    /// Whether it keeps the blank line before it, which comes right after an attribute's line.
    /// Nu goes on past a blank line to a line-start `|` only there.
    /// Without the blank line, nu would take that `|` into the attribute.
    fn keeps_blank_line_after_attribute(&self) -> bool {
        let NodeOrToken::Token(token) = &self.child.element else {
            return false;
        };
        let before = iter::successors(token.prev_sibling_or_token(), |element| {
            element.prev_sibling_or_token()
        })
        .find(|element| !element.kind().is_trivia());
        self.child.newlines > 1
            && before
                .and_then(NodeOrToken::into_node)
                .is_some_and(|node| headed_by_attribute(&node))
    }

    fn is_redirection_pipe(&self) -> bool {
        matches!(
            self.kind(),
            SyntaxKind::ErrGtPipe | SyntaxKind::OutErrGtPipe
        )
    }
}

/// An `Assignment` or a `Let` is laid flat, unless what holds it is kept as written.
/// The pipes of the first statement the operator takes then format as those of `pipeline`.
/// The statements below it are left to `assigned`.
fn parts(pipeline: &SyntaxNode, kept: &Kept) -> Vec<Part> {
    let levels = Levels {
        head: 0,
        pipes: 0,
    };
    comments_at_level_of_following(flatten(pipeline, 0, false, levels, kept))
}

/// A comment on a line of its own is about what follows it, so it takes that indent.
/// `flatten` gives the deeper indent of a value's `|` only to the `|`, not to comments before it.
fn comments_at_level_of_following(parts: Vec<Part>) -> Vec<Part> {
    let levels: Vec<_> = parts
        .iter()
        .enumerate()
        .map(|(index, part)| {
            let following = parts[index..].iter().find(|part| !part.child.is_comment());
            following.unwrap_or(part).level
        })
        .collect();
    parts
        .into_iter()
        .zip(levels)
        .map(|(part, level)| Part {
            level,
            ..part
        })
        .collect()
}

/// The indent levels the parts of a pipeline take when they start a line.
/// A value starts on the line of its assignment, so only its `|`s go deeper.
#[derive(Clone, Copy)]
struct Levels {
    /// The level of the parts before the pipeline's first `|`.
    head: u32,
    /// The level of the parts from the pipeline's first `|` on.
    pipes: u32,
}

fn assignment_of(pipeline: &SyntaxNode, kept: &Kept) -> Option<SyntaxNode> {
    pipeline
        .children()
        .filter(|element| !kept.keeps(&element.clone().into()))
        .filter_map(|element| element.first_child())
        .find(is_assignment)
}

fn is_assignment(node: &SyntaxNode) -> bool {
    matches!(node.kind(), SyntaxKind::Assignment | SyntaxKind::Let)
}

fn assigned_children(assignment: &SyntaxNode, kept: &Kept) -> (Vec<Child>, Option<usize>) {
    let items: Vec<_> = children(assignment.children_with_tokens(), kept).collect();
    let first = items.iter().position(Child::is_statement);
    (items, first)
}

fn flatten(
    pipeline: &SyntaxNode,
    newlines: u32,
    joined: bool,
    levels: Levels,
    kept: &Kept,
) -> Vec<Part> {
    let children: Vec<_> = children(pipeline.children_with_tokens(), kept).collect();
    let first_pipe = children
        .iter()
        .position(|child| child.kind() == SyntaxKind::Pipe)
        .unwrap_or(children.len());
    children
        .into_iter()
        .enumerate()
        .flat_map(|(index, child)| {
            let level = if index < first_pipe {
                levels.head
            } else {
                levels.pipes
            };
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
                .filter(|_| !kept.keeps(&ast::untyped(&child.element)))
                .and_then(|node| node.syntax().first_child())
                .filter(is_assignment);
            match assignment {
                Some(assignment) => {
                    flatten_assignment(&assignment, child.newlines, joined, level, kept)
                },
                None => vec![Part {
                    joined: joined && child.element.as_node().is_some(),
                    child,
                    level,
                }],
            }
        })
        .collect()
}

fn flatten_assignment(
    assignment: &SyntaxNode,
    newlines: u32,
    joined: bool,
    level: u32,
    kept: &Kept,
) -> Vec<Part> {
    let (items, first) = assigned_children(assignment, kept);
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
                NodeOrToken::Node(Node::Pipeline(pipeline))
                    if !kept.keeps(&ast::untyped(&child.element)) =>
                {
                    let levels = Levels {
                        head: level,
                        pipes: level + 1,
                    };
                    flatten(pipeline.syntax(), child.newlines, true, levels, kept)
                },
                _ => vec![Part {
                    joined: joined && !child.is_comment(),
                    child,
                    level,
                }],
            }
        })
        .collect()
}

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

fn keeps_pipe_at_line_end(items: &[Part]) -> bool {
    items
        .first()
        .is_some_and(|part| part.child.is_comment() || part.starts_line_with_attribute())
}

/// `breaks_at_operators` tells an element that breaks at its operators, which is no tail.
fn segments(
    parts: &[Part],
    after_semicolon: bool,
    breaks_at_operators: impl FnOnce(&Part) -> bool,
) -> (Vec<Segment<'_>>, Option<Segment<'_>>) {
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
    let (last, tail) = last.split_tail(breaks_at_operators);
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
        let align = if keeps_pipe_at_line_end(items) || (prev.is_none() && after_semicolon) {
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

    fn keeps_pipe_at_line_end(&self) -> bool {
        keeps_pipe_at_line_end(self.items)
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

    fn split_tail(&self, breaks_at_operators: impl FnOnce(&Part) -> bool) -> (Self, Option<Self>) {
        let last = self.items.iter().rposition(|part| !part.joined);
        // An element that an attribute heads keeps the `|` before it at the end of the line.
        // Only the segment that holds that `|` can, so the element is no tail.
        // An expression that breaks at its operators is no tail either, so the `|`s break first.
        let Some(tail) = last.filter(|&last| {
            let part = &self.items[last];
            part.is_element() && !part.starts_line_with_attribute() && !breaks_at_operators(part)
        }) else {
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
