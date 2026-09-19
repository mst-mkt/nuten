use nuten_doc::Element;
use nuten_syntax::ast::{AstNode, MatchArm};

use super::{Formatter, children};

impl<'a> Formatter<'a> {
    pub(super) fn arm(&mut self, arm: &MatchArm) -> Vec<Element<'a>> {
        let items: Vec<_> = children(arm.syntax().children_with_tokens()).collect();
        self.spaced_children(&items, |_, _| false)
    }
}
