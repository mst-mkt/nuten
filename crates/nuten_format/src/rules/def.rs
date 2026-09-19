use nuten_doc::{Element, expand_parent, group, space};
use nuten_syntax::{
    NodeOrToken, SyntaxKind,
    ast::{self, AstNode, Def, Node},
};

use super::{Formatter, block::BlockLayout, children};

impl<'a> Formatter<'a> {
    /// A `def` or an `extern`: its parts one space apart, with the `:` the input and output types follow right after the parameters, and the body expanded.
    pub(super) fn def(&mut self, def: &Def) -> Vec<Element<'a>> {
        let items: Vec<_> = children(def.syntax().children_with_tokens())
            .map(|child| child.element)
            .collect();
        let body = def.body();
        let mut out = Vec::new();
        for (index, item) in items.iter().enumerate() {
            // Another `:` after it would make nu read the types from elsewhere.
            let joined = ast::kind_of(item) == SyntaxKind::Colon
                && index > 0
                && ast::kind_of(&items[index - 1]) == SyntaxKind::Signature
                && items
                    .get(index + 1)
                    .is_none_or(|next| next.as_token().is_none_or(|next| next.text() != ":"));
            if !out.is_empty() && !joined {
                out.push(space());
            }
            match item {
                NodeOrToken::Node(Node::Block(block)) if Some(block) == body.as_ref() => {
                    let BlockLayout {
                        mut head,
                        mut rest,
                        ..
                    } = self.block_layout(block);
                    rest.push(expand_parent());
                    head.push(group(rest));
                    out.extend(head);
                },
                _ => out.extend(self.element(item)),
            }
        }
        out
    }
}
