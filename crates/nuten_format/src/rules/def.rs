use nuten_doc::{Element, expand_parent, group, space};
use nuten_syntax::{SyntaxElement, SyntaxKind, SyntaxNode};

use super::{
    block::{BlockLayout, block_layout},
    children, node, token,
};

/// A `def` or an `extern`: its parts one space apart, with the `:` the input
/// and output types follow right after the parameters, and the body expanded.
pub(super) fn def<'a>(src: &'a str, def: &SyntaxNode) -> Vec<Element<'a>> {
    let items: Vec<_> = children(def).map(|child| child.element).collect();
    let body = def
        .last_child()
        .filter(|last| last.kind() == SyntaxKind::Block);
    let mut out = Vec::new();
    for (index, item) in items.iter().enumerate() {
        // Another `:` after it would make nu read the types from elsewhere.
        let joined = item.kind() == SyntaxKind::Colon
            && index > 0
            && items[index - 1].kind() == SyntaxKind::Signature
            && items
                .get(index + 1)
                .is_none_or(|next| next.as_token().is_none_or(|next| next.text() != ":"));
        if !out.is_empty() && !joined {
            out.push(space());
        }
        match item {
            SyntaxElement::Node(child) if Some(child) == body.as_ref() => {
                let BlockLayout {
                    mut head,
                    mut rest,
                    ..
                } = block_layout(src, child);
                rest.push(expand_parent());
                head.push(group(rest));
                out.extend(head);
            },
            SyntaxElement::Node(child) => out.extend(node(src, child)),
            SyntaxElement::Token(child) => out.push(token(src, child)),
        }
    }
    out
}
