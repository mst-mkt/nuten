use nuten_doc::{
    Build, empty_line, expand_parent, hard_line_break, soft_line_break_or_space, space, text,
};
use nuten_syntax::{
    NodeOrToken, SyntaxElement, SyntaxKind,
    ast::{self, AstNode, Block, Bracketed, ClosureParams, Node},
};

use super::{Child, Formatter, children, ends_with_pipe, kept::Kept, without_separators};

impl Formatter<'_> {
    pub(super) fn block(&mut self, block: &Block) {
        self.grouped_block(block, false);
    }

    pub(super) fn element_or_body(&mut self, element: &ast::Element, body: Option<&Block>) {
        match element {
            NodeOrToken::Node(Node::Block(block))
                if Some(block) == body && !self.kept.keeps(&ast::untyped(element)) =>
            {
                self.grouped_block(block, true);
            },
            element => self.element(element),
        }
    }

    /// The head stays out of the group.
    /// Parameters over lines then leave the rest to fit after their last line.
    fn grouped_block(&mut self, block: &Block, always_breaks: bool) {
        let body: Vec<_> = children(block.statements(), &self.kept).collect();
        let breaks = always_breaks || breaks_lines(block, &body);
        self.block_head(block);
        self.group(|this| {
            this.block_body(block, &body);
            if breaks {
                this.push(expand_parent());
            }
        });
    }

    pub(super) fn ungrouped_block(&mut self, block: &Block) {
        let body: Vec<_> = children(block.statements(), &self.kept).collect();
        self.block_head(block);
        self.block_body(block, &body);
    }

    fn block_head(&mut self, block: &Block) {
        self.push(self.token(&block.open()));
        if let Some(params) = block.params() {
            self.closure_params(&params);
        }
    }

    fn block_body(&mut self, block: &Block, body: &[Child]) {
        let close = self.token(&block.close());
        let Some(first) = body.first() else {
            self.push(close);
            return;
        };
        let lead = if first.is_comment() && first.newlines == 0 {
            None
        } else if block.params().is_none()
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
        self.indent(|this| {
            this.extend(lead);
            this.statements(body, false);
        });
        // Nu needs a line break after a `|` that ends the block.
        let before_close = if body
            .last()
            .and_then(|last| last.element.as_node())
            .is_some_and(|statement| ends_with_pipe(statement.syntax()))
        {
            hard_line_break()
        } else {
            soft_line_break_or_space()
        };
        self.extend([before_close, close]);
    }

    /// A space stays between two `|` with nothing between them, since `||` is lexed as one token.
    fn closure_params(&mut self, params: &ClosureParams) {
        let syntax = params.syntax();
        if closure_params_stay_as_is(params, &self.kept) {
            self.source(syntax.text_range());
            return;
        }
        let items: Vec<_> =
            without_separators(children(syntax.children_with_tokens(), &self.kept)).collect();
        for (index, item) in items.iter().enumerate() {
            let prev = index.checked_sub(1).map(|prev| items[prev].kind());
            match (prev, item.kind()) {
                (Some(SyntaxKind::Param), SyntaxKind::Param) => {
                    if self.commas.on_one_line() {
                        self.push(text(","));
                    }
                    self.push(space());
                },
                (Some(SyntaxKind::Pipe), SyntaxKind::Pipe) => self.push(space()),
                _ => {},
            }
            self.element(&item.element);
        }
    }
}

fn breaks_lines(block: &Block, body: &[Child]) -> bool {
    // A comment the `}` follows on its line ends at a lone `\r`.
    // As a line suffix it would move past the `}`.
    let holds_comment = body.iter().any(Child::is_comment);
    let over_lines = block
        .statements()
        .any(|element| element.kind() == SyntaxKind::Newline);
    holds_comment || (!body.is_empty() && over_lines)
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
