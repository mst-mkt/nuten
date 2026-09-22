use nuten_doc::{Element, expand_parent, group};
use nuten_syntax::{
    NodeOrToken, SyntaxKind,
    ast::{AstNode, Def, Node},
};

use super::{Child, Formatter, block::BlockLayout, children, line_break_or_space, separator};

impl<'a> Formatter<'a> {
    pub(super) fn def(&mut self, def: &Def) -> Vec<Element<'a>> {
        let items: Vec<_> = children(def.syntax().children_with_tokens()).collect();
        let body = def.body();
        let mut out = Vec::new();
        for (index, item) in items.iter().enumerate() {
            if let Some(comment) = item.as_comment() {
                out.extend(self.comment(comment, item.newlines, false));
                continue;
            }
            let prev = items[..index].last().map(Child::kind);
            // Another `:` after it would make nu read the types from elsewhere.
            let joined = item.kind() == SyntaxKind::Colon
                && prev == Some(SyntaxKind::Signature)
                && !next_is_colon(&items[index + 1..]);
            let plain = (!joined).then(|| line_break_or_space(item));
            out.extend(separator(prev, plain));
            match &item.element {
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
                element => out.extend(self.element(element)),
            }
        }
        out
    }
}

/// Only the `:` right after the parameters is a `Colon`, so a second one stays a `Word`.
fn next_is_colon(rest: &[Child]) -> bool {
    rest.iter()
        .find(|child| !child.is_comment())
        .and_then(|child| child.element.as_token())
        .is_some_and(|token| token.text() == ":")
}
