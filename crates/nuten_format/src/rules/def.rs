use nuten_doc::{Element, soft_line_break, space};
use nuten_syntax::{
    NodeOrToken, SyntaxElement, SyntaxKind, SyntaxToken,
    ast::{AstNode, Def, IoType, IoTypes},
};

use super::{Child, Formatter, children, line_break_or_space};

impl<'a> Formatter<'a> {
    pub(super) fn def(&mut self, def: &Def) -> Vec<Element<'a>> {
        let items: Vec<_> = children(def.syntax().children_with_tokens()).collect();
        let body = def.body();
        let has_body = has_body(def);
        self.spaced_children_by(
            &items,
            |index, item| {
                let joined = item.kind() == SyntaxKind::Colon
                    && items[..index].last().map(Child::kind) == Some(SyntaxKind::Signature)
                    && joins_parameters(item, &items[index + 1..], has_body);
                (!joined).then(|| line_break_or_space(item))
            },
            |this, element| this.element_or_body(element, body.as_ref()),
        )
    }

    pub(super) fn io_types(&mut self, types: &IoTypes) -> Vec<Element<'a>> {
        let syntax = types.syntax();
        if holds_joining_comment(types) {
            return vec![self.source(syntax.text_range())];
        }
        match types.brackets() {
            Some((open, inside, close)) => {
                let head = vec![self.token(&open)];
                self.items(
                    head,
                    children(inside),
                    false,
                    soft_line_break,
                    soft_line_break,
                    &close,
                    true,
                )
            },
            None => self.comma_spaced(children(syntax.children_with_tokens())),
        }
    }

    pub(super) fn io_type(&mut self, pair: &IoType) -> Vec<Element<'a>> {
        let items: Vec<_> = children(pair.syntax().children_with_tokens()).collect();
        self.spaced_children(&items, |_, _| Some(space()))
    }
}

/// Whether a comment stands inside a pair, or among pairs with no `[ ]` to break the line in.
/// Printed on one line with the pairs, it would take in what follows it.
fn holds_joining_comment(types: &IoTypes) -> bool {
    let bare = types.brackets().is_none();
    types
        .syntax()
        .children_with_tokens()
        .any(|child| match child {
            NodeOrToken::Node(pair) => pair
                .children_with_tokens()
                .any(|part| part.kind() == SyntaxKind::Comment),
            NodeOrToken::Token(token) => bare && token.kind() == SyntaxKind::Comment,
        })
}

/// Whether the `:` after the parameters joins them, `rest` being what follows it.
/// Nu reads `[]:` with no types after it, or only one item in braces, as the whole signature.
/// In those cases nu reads `[] :` as parameters with no types, so the `:` stays as written.
/// Nu takes the last item of a `def` for its body, whatever the parser makes of it.
/// An `IoTypes` is one child, but never one item to nu.
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
        [only] => {
            only.kind() == SyntaxKind::IoTypes
                || !first_token(only).is_some_and(|token| token.text().starts_with('{'))
        },
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
