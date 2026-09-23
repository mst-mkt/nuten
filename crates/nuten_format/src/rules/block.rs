use nuten_doc::{
    Element, empty_line, expand_parent, group, indent, soft_line_break_or_space, space, text,
};
use nuten_syntax::{
    NodeOrToken, SyntaxElement, SyntaxKind,
    ast::{self, AstNode, Block, Bracketed, ClosureParams, Node},
};

use super::{Child, Formatter, children, without_commas};

/// A block laid out for its container to group.
pub(super) struct BlockLayout<'a> {
    /// The opening brace and the parameters, which stay out of the group.
    /// Parameters over lines then leave the rest to fit after their last line.
    pub(super) head: Vec<Element<'a>>,
    pub(super) rest: Vec<Element<'a>>,
    /// Whether a line break comes after the parameters in the source.
    line_break: bool,
    /// Whether it holds at most one statement and nothing else.
    pub(super) one_statement: bool,
}

impl<'a> Formatter<'a> {
    pub(super) fn block(&mut self, block: &Block) -> Vec<Element<'a>> {
        let BlockLayout {
            mut head,
            mut rest,
            line_break,
            ..
        } = self.block_layout(block);
        if line_break {
            rest.push(expand_parent());
        }
        head.push(group(rest));
        head
    }

    /// `element`, or, where it is `body`, the body broken over lines whatever it holds.
    pub(super) fn element_or_body(
        &mut self,
        element: &ast::Element,
        body: Option<&Block>,
    ) -> Vec<Element<'a>> {
        match element {
            NodeOrToken::Node(Node::Block(block)) if Some(block) == body => {
                let BlockLayout {
                    mut head,
                    mut rest,
                    ..
                } = self.block_layout(block);
                rest.push(expand_parent());
                head.push(group(rest));
                head
            },
            element => self.element(element),
        }
    }

    pub(super) fn block_layout(&mut self, block: &Block) -> BlockLayout<'a> {
        let mut head = vec![self.token(&block.open())];
        let params = block.params();
        if let Some(params) = &params {
            head.extend(self.closure_params(params));
        }
        let close = self.token(&block.close());
        let body: Vec<_> = children(block.statements()).collect();
        let one_statement = matches!(
            body.as_slice(),
            [] | [Child {
                element: NodeOrToken::Node(_),
                ..
            }]
        );
        let Some(first) = body.first() else {
            return BlockLayout {
                head,
                rest: vec![close],
                line_break: false,
                one_statement,
            };
        };
        let line_break = block
            .statements()
            .any(|element| element.kind() == SyntaxKind::Newline);
        let lead = if first.is_comment() && first.newlines == 0 {
            None
        } else if params.is_none()
            && first
                .element
                .as_node()
                .and_then(|statement| statement.syntax().first_child_or_token())
                .is_some_and(|first| first.kind() == SyntaxKind::Pipe)
        {
            // Right after the brace and a line break, nu would read the `|` as parameters.
            Some(empty_line())
        } else {
            Some(soft_line_break_or_space())
        };
        let inner = lead.into_iter().chain(self.statements(&body, false));
        BlockLayout {
            head,
            rest: vec![indent(inner), soft_line_break_or_space(), close],
            line_break,
            one_statement,
        }
    }

    /// A space stays between two `|` with nothing between them, since `||` is lexed as one token.
    fn closure_params(&mut self, params: &ClosureParams) -> Vec<Element<'a>> {
        let syntax = params.syntax();
        if closure_params_stay_as_is(params) {
            return vec![self.source(syntax.text_range())];
        }
        let items: Vec<_> = without_commas(children(syntax.children_with_tokens())).collect();
        items
            .iter()
            .enumerate()
            .flat_map(|(index, item)| {
                let prev = index.checked_sub(1).map(|prev| items[prev].kind());
                let separator = match (prev, item.kind()) {
                    (Some(SyntaxKind::Param), SyntaxKind::Param) => vec![text(","), space()],
                    (Some(SyntaxKind::Pipe), SyntaxKind::Pipe) => vec![space()],
                    _ => Vec::new(),
                };
                separator.into_iter().chain(self.element(&item.element))
            })
            .collect()
    }
}

/// Whether the parameters stay as they are.
fn closure_params_stay_as_is(params: &ClosureParams) -> bool {
    holds_stray_token(params) || holds_ignored_token(params)
}

/// Whether something between the pipes, other than a `,`, is outside any `Param`.
/// The layout would join it to what follows it.
fn holds_stray_token(params: &ClosureParams) -> bool {
    without_commas(children(params.syntax().children_with_tokens())).any(|child| {
        !matches!(
            child.kind(),
            SyntaxKind::Pipe | SyntaxKind::PipePipe | SyntaxKind::Param
        )
    })
}

/// Whether the parameters hold a token that nu ignores, such as a comment.
/// A comment runs to the end of its line.
/// A redirection nu ignores would take the `|` after it as its own, as in `e>|`.
fn holds_ignored_token(params: &ClosureParams) -> bool {
    params
        .syntax()
        .descendants_with_tokens()
        .filter_map(SyntaxElement::into_token)
        .any(|token| {
            let kind = token.kind();
            !kind.is_item()
                && !matches!(
                    kind,
                    SyntaxKind::Pipe
                        | SyntaxKind::PipePipe
                        | SyntaxKind::Colon
                        | SyntaxKind::Eq
                        | SyntaxKind::Comma
                        | SyntaxKind::ShortFlag
                        | SyntaxKind::LAngle
                        | SyntaxKind::RAngle
                        | SyntaxKind::Whitespace
                        | SyntaxKind::Newline
                )
        })
}
