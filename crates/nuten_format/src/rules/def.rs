use nuten_doc::{Build, soft_line_break_or_space, space};
use nuten_syntax::{
    NodeOrToken, SyntaxElement, SyntaxKind, SyntaxToken,
    ast::{AstNode, Def, IoType, IoTypes},
};

use super::{Child, Formatter, children, container::ItemsLayout, line_break_or_space};
use crate::Commas;

impl Formatter<'_> {
    pub(super) fn def(&mut self, def: &Def) {
        let items: Vec<_> = children(def.syntax().children_with_tokens(), &self.kept).collect();
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
            |this, _, element| this.element_or_body(element, body.as_ref()),
        );
    }

    pub(super) fn io_types(&mut self, types: &IoTypes) {
        let syntax = types.syntax();
        if holds_joining_comment(types) {
            self.source(syntax.text_range());
            return;
        }
        // A `,` between the pairs whatever `commas` says, since `->` leaves no cue where one ends.
        match types.brackets() {
            // A line break before the `]` would part it from the last type.
            // Breaking between the pairs while the `]` stays on the last line looks like a mistake.
            // So a list holding a type that cannot be flat stays as written.
            Some((open, inside, close)) if touches_last_type(&close) => self.flat_or(
                |this| {
                    this.push(this.token(&open));
                    this.one_line_items(inside, |_| true);
                    this.push(this.token(&close));
                },
                |this| this.source(syntax.text_range()),
            ),
            Some((open, inside, close)) => {
                self.items(
                    |this| this.push(this.token(&open)),
                    inside,
                    &close,
                    ItemsLayout {
                        expand: false,
                        before_first: soft_line_break_or_space,
                        before_close: soft_line_break_or_space,
                        commas: Commas::Always,
                    },
                );
            },
            None => self.one_line_items(syntax.children_with_tokens(), |_| true),
        }
    }

    pub(super) fn io_type(&mut self, pair: &IoType) {
        let items: Vec<_> = children(pair.syntax().children_with_tokens(), &self.kept).collect();
        self.spaced_children(&items, |_, _| Some(space()));
    }
}

/// Printed on one line with the pairs, such a comment would take in what follows it.
fn holds_joining_comment(types: &IoTypes) -> bool {
    let unbroken = types
        .brackets()
        .is_none_or(|(_, _, close)| touches_last_type(&close));
    types
        .syntax()
        .children_with_tokens()
        .any(|child| match child {
            NodeOrToken::Node(pair) => pair
                .children_with_tokens()
                .any(|part| part.kind() == SyntaxKind::Comment),
            NodeOrToken::Token(token) => unbroken && token.kind() == SyntaxKind::Comment,
        })
}

/// Where they touch, nu reads the type and the `]` as one word, `any`, for a call above the `def`.
/// So whether they touch stays as written while nuten supports a nu that reads them so.
fn touches_last_type(close: &SyntaxToken) -> bool {
    close
        .prev_sibling_or_token()
        .is_some_and(|prev| prev.kind() == SyntaxKind::IoType)
}

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

fn has_body(def: &Def) -> bool {
    def.syntax()
        .children_with_tokens()
        .filter_map(SyntaxElement::into_token)
        .find(|token| !token.kind().is_trivia() && token.text() != "export")
        .is_some_and(|token| token.text() == "def")
}
