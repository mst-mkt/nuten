use std::iter;

use nuten_doc::{
    Build, GroupId, empty_line, expand_parent, hard_line_break, soft_line_break_or_space, space,
};
use nuten_syntax::{
    NodeOrToken, SyntaxKind, SyntaxNode,
    ast::{self, AstNode, Node, Pipeline},
};

use super::{Child, Formatter, children, headed_by_attribute, kept::Kept, separator};

impl Formatter<'_> {
    pub(super) fn pipeline(&mut self, pipeline: &Pipeline) {
        let id = self.groups.fresh();
        let parts = parts(pipeline.syntax(), &self.kept);
        let expand = parts.iter().any(|part| part.child.newlines > 0);
        let (segments, tail) = segments(&parts, follows_semicolon(pipeline.syntax()), |part| {
            part.child.element.as_node().is_some_and(|node| {
                self.holds_expression_breaking_at_operators(node.syntax())
                    || self.holds_call_breaking_at_arguments(node.syntax())
            })
        });
        self.group_with_id(id, |this| {
            if expand {
                this.push(expand_parent());
            }
            for segment in &segments {
                this.segment(segment, id);
            }
        });
        // The tail is measured in break mode.
        // Its first line break then ends the measure instead of breaking the group.
        if let Some(tail) = tail {
            self.segment(&tail, id);
        }
        self.assigned(pipeline.syntax());
    }

    fn assigned(&mut self, pipeline: &SyntaxNode) {
        let assigned = assignment_of(pipeline, &self.kept)
            .map(|assignment| assigned_children(&assignment, &self.kept));
        let Some((items, Some(first))) = assigned else {
            return;
        };
        for index in first + 1..items.len() {
            self.statement(&items[..index], &items[index], false);
        }
    }

    fn segment(&mut self, segment: &Segment<'_>, id: GroupId) {
        let end = segment.aligned_end();
        let level = segment
            .pipe
            .or(segment.items.first())
            .map_or(0, |part| part.level);
        self.indent_levels_if_group_breaks(id, level, |this| {
            this.pipe(segment);
            this.aligned(segment.align, id, |this| {
                for (index, part) in segment.items[..end].iter().enumerate() {
                    this.part(segment.prev_of(index), part);
                }
            });
        });
        // From the first comment on, each part may start a line of its own.
        // So each takes its own indent, rather than that of the segment.
        for (index, part) in segment.items[end..].iter().enumerate() {
            self.indent_levels_if_group_breaks(id, part.level, |this| {
                this.part(segment.prev_of(end + index), part);
            });
        }
    }

    fn aligned(&mut self, align: Option<Alignment>, id: GroupId, write: impl FnOnce(&mut Self)) {
        match align {
            Some(Alignment::Always) => self.align(2, write),
            Some(Alignment::IfGroupBreaks) => self.align_if_group_breaks(id, 2, write),
            None => write(self),
        }
    }

    fn pipe(&mut self, segment: &Segment<'_>) {
        let Some(pipe) = segment.pipe else {
            return;
        };
        let gap = if pipe.keeps_blank_line_after_attribute() {
            // Not through `separator`, which leaves only a line break after a comment.
            Some(empty_line())
        } else {
            // A pipe before an attribute stays, since nu reads one only after a line break.
            let plain = if segment.keeps_pipe_at_line_end() {
                space()
            } else {
                soft_line_break_or_space()
            };
            separator(segment.prev, Some(plain))
        };
        self.extend(gap);
        self.element(&pipe.child.element);
    }

    fn part(&mut self, prev: Option<SyntaxKind>, part: &Part) {
        let Child {
            element,
            newlines,
        } = &part.child;
        match element {
            NodeOrToken::Token(comment) if comment.kind() == SyntaxKind::Comment => {
                if part.keeps_blank_line_after_attribute() {
                    self.push(empty_line());
                }
                self.comment(comment, *newlines, prev.is_none());
            },
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
                self.extend(separator(prev, Some(plain)));
                self.element(element);
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

struct Part {
    child: Child,
    joined: bool,
    level: u32,
}

impl Part {
    fn kind(&self) -> SyntaxKind {
        self.child.kind()
    }

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

/// A value starts on the line of its assignment, so only its `|`s go deeper.
#[derive(Clone, Copy)]
struct Levels {
    head: u32,
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
    Always,
    IfGroupBreaks,
}

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

fn segments(
    parts: &[Part],
    after_semicolon: bool,
    breaks_itself: impl FnOnce(&Part) -> bool,
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
    let (last, tail) = last.split_tail(breaks_itself);
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

    fn split_tail(&self, breaks_itself: impl FnOnce(&Part) -> bool) -> (Self, Option<Self>) {
        let last = self.items.iter().rposition(|part| !part.joined);
        // An element that an attribute heads keeps the `|` before it at the end of the line.
        // Only the segment that holds that `|` can, so the element is no tail.
        // An element that breaks inside itself is no tail either, so the `|`s break first.
        let Some(tail) = last.filter(|&last| {
            let part = &self.items[last];
            part.is_element() && !part.starts_line_with_attribute() && !breaks_itself(part)
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
