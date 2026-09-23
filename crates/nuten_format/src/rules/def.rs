use nuten_doc::{Element, expand_parent, group};
use nuten_syntax::{
    NodeOrToken, SyntaxElement, SyntaxKind, SyntaxToken,
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
            let joined = item.kind() == SyntaxKind::Colon
                && prev == Some(SyntaxKind::Signature)
                && joins_parameters(item, &items[index + 1..], has_body(def));
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

/// Whether the `:` after the parameters joins them, `rest` being what follows it.
/// Nu reads `[]:` with no types after it, or only one item in braces, as the whole signature.
/// In those cases nu reads `[] :` as parameters with no types, so the `:` stays as written.
/// Nu takes the last item of a `def` for its body, whatever the parser makes of it.
/// A second `:` starts types nu rejects, so the `:` stays as written before it too.
/// Only the `:` right after the parameters is a `Colon`, so a second one stays a `Word`.
fn joins_parameters(colon: &Child, rest: &[Child], has_body: bool) -> bool {
    let attached = colon
        .element
        .as_token()
        .and_then(SyntaxToken::prev_sibling_or_token)
        .is_some_and(|prev| prev.kind() == SyntaxKind::Signature);
    let following: Vec<_> = rest.iter().filter(|child| !child.is_comment()).collect();
    let types = match following.split_last() {
        Some((_, types)) if has_body => types,
        _ => &following,
    };
    let same_either_way = match types {
        [] => false,
        [first, ..] if first_token(first).is_some_and(|token| token.text() == ":") => false,
        [only] => !first_token(only).is_some_and(|token| token.text().starts_with('{')),
        _ => true,
    };
    attached || same_either_way
}

fn first_token(child: &Child) -> Option<SyntaxToken> {
    match &child.element {
        NodeOrToken::Node(node) => node.syntax().first_token(),
        NodeOrToken::Token(token) => Some(token.clone()),
    }
}

/// Whether the keyword, after any `export`, is `def` rather than `extern`, which has no body.
fn has_body(def: &Def) -> bool {
    def.syntax()
        .children_with_tokens()
        .filter_map(SyntaxElement::into_token)
        .find(|token| !token.kind().is_trivia() && token.text() != "export")
        .is_some_and(|token| token.text() == "def")
}
