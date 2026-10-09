use nuten_doc::space;
use nuten_syntax::ast::{AstNode, MatchArm};

use super::{Formatter, children};

impl Formatter<'_> {
    pub(super) fn arm(&mut self, arm: &MatchArm) {
        let items: Vec<_> = children(arm.syntax().children_with_tokens(), &self.kept).collect();
        self.spaced_children(&items, |_, _| Some(space()));
    }
}
