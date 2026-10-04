use nuten_doc::{
    Element, empty_line, expand_parent, group, indent, soft_line_break_or_space, space, text,
};
use nuten_syntax::{
    NodeOrToken, SyntaxElement, SyntaxKind,
    ast::{self, AstNode, Block, Bracketed, ClosureParams, Node},
};

use super::{Child, Formatter, children, kept::Kept, without_separators};

pub(super) struct BlockLayout<'a> {
    /// Out of the group, so that parameters over lines leave the rest to fit after their last line.
    pub(super) head: Vec<Element<'a>>,
    pub(super) rest: Vec<Element<'a>>,
    /// A comment the `}` follows on its line ends at a lone `\r`.
    /// As a line suffix it would move past the `}`.
    expand: bool,
}

impl<'a> BlockLayout<'a> {
    fn grouped(self) -> Vec<Element<'a>> {
        let rest = self.rest.into_iter().chain(self.expand.then(expand_parent));
        self.head.into_iter().chain([group(rest)]).collect()
    }
}

impl<'a> Formatter<'a> {
    pub(super) fn block(&mut self, block: &Block) -> Vec<Element<'a>> {
        self.block_layout(block).grouped()
    }

    pub(super) fn element_or_body(
        &mut self,
        element: &ast::Element,
        body: Option<&Block>,
    ) -> Vec<Element<'a>> {
        match element {
            NodeOrToken::Node(Node::Block(block))
                if Some(block) == body && !self.kept.keeps(&ast::untyped(element)) =>
            {
                BlockLayout {
                    expand: true,
                    ..self.block_layout(block)
                }
                .grouped()
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
        let body: Vec<_> = children(block.statements(), &self.kept).collect();
        let Some(first) = body.first() else {
            return BlockLayout {
                head,
                rest: vec![close],
                expand: false,
            };
        };
        let expand = body.iter().any(Child::is_comment)
            || block
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
            expand,
        }
    }

    /// A space stays between two `|` with nothing between them, since `||` is lexed as one token.
    fn closure_params(&mut self, params: &ClosureParams) -> Vec<Element<'a>> {
        let syntax = params.syntax();
        if closure_params_stay_as_is(params, &self.kept) {
            return self.source(syntax.text_range());
        }
        let items: Vec<_> =
            without_separators(children(syntax.children_with_tokens(), &self.kept)).collect();
        items
            .iter()
            .enumerate()
            .flat_map(|(index, item)| {
                let prev = index.checked_sub(1).map(|prev| items[prev].kind());
                let separator = match (prev, item.kind()) {
                    (Some(SyntaxKind::Param), SyntaxKind::Param) if self.commas.on_one_line() => {
                        vec![text(","), space()]
                    },
                    (Some(SyntaxKind::Param), SyntaxKind::Param)
                    | (Some(SyntaxKind::Pipe), SyntaxKind::Pipe) => vec![space()],
                    _ => Vec::new(),
                };
                separator.into_iter().chain(self.element(&item.element))
            })
            .collect()
    }
}

pub(super) fn holds_at_most_one_statement(block: &Block, kept: &Kept) -> bool {
    let body: Vec<_> = children(block.statements(), kept).collect();
    matches!(
        body.as_slice(),
        [] | [Child {
            element: NodeOrToken::Node(_),
            ..
        }]
    )
}

fn closure_params_stay_as_is(params: &ClosureParams, kept: &Kept) -> bool {
    holds_stray_token(params, kept) || holds_ignored_token(params)
}

/// The layout would join such a token to what follows it.
fn holds_stray_token(params: &ClosureParams, kept: &Kept) -> bool {
    without_separators(children(params.syntax().children_with_tokens(), kept)).any(|child| {
        !matches!(
            child.kind(),
            SyntaxKind::Pipe | SyntaxKind::PipePipe | SyntaxKind::Param
        )
    })
}

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
