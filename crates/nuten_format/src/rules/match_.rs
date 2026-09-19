use nuten_doc::Element;
use nuten_syntax::ast::{AstNode, MatchArm};

use super::{Formatter, children};

impl<'a> Formatter<'a> {
    /// An arm: the tokens of its pattern one space apart, whatever line breaks came among them, ` => `, and what it does, a block as the block rule lays it out.
    /// A comment stays, and what follows it starts a line.
    pub(super) fn arm(&mut self, arm: &MatchArm) -> Vec<Element<'a>> {
        let items: Vec<_> = children(arm.syntax().children_with_tokens()).collect();
        self.spaced_children(&items, |_, _| false)
    }
}
