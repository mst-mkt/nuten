use nuten_doc::{Build, space};
use nuten_syntax::{
    SyntaxKind,
    ast::{AstNode, Bracketed, Param, Type, TypeField},
};

use super::{Formatter, children, without_separators};

impl Formatter<'_> {
    pub(super) fn param(&mut self, param: &Param) {
        let items: Vec<_> =
            without_separators(children(param.syntax().children_with_tokens(), &self.kept))
                .collect();
        let name_is_long_flag = items
            .first()
            .and_then(|name| name.element.as_token())
            .is_some_and(|name| is_long_flag(name.text()));
        self.spaced_children(&items, |index, item| match item.kind() {
            // A `:` after a comment stays on the next line all the same.
            // Joined, it would land inside the comment.
            // As a line suffix, the comment would land after the closing bracket.
            // Nu would lose the description either way.
            SyntaxKind::Colon => None,
            // A `(-f)` joins the name it follows, a long flag.
            // Joined to a type or a default value, it would become part of that.
            SyntaxKind::ShortFlag if index == 1 && name_is_long_flag => None,
            _ => Some(space()),
        });
    }

    /// No comment comes inside, since the parser keeps a type holding one as one token.
    pub(super) fn type_(&mut self, type_: &Type) {
        self.extend([self.token(&type_.name()), self.token(&type_.open())]);
        let takes_commas = self.commas.on_one_line();
        // Without the `,`, nu would read the `:` as that of a key before it.
        self.one_line_items(type_.inside(), |param| {
            takes_commas
                || param
                    .element
                    .as_node()
                    .and_then(|param| param.syntax().first_token())
                    .is_some_and(|first| first.kind() == SyntaxKind::Colon)
        });
        self.push(self.token(&type_.close()));
    }

    /// A `,` here is the type nu takes for the key, so it stays, unlike one among the fields.
    pub(super) fn type_field(&mut self, field: &TypeField) {
        let items: Vec<_> = children(field.syntax().children_with_tokens(), &self.kept).collect();
        self.spaced_children(&items, |_, item| {
            (item.kind() != SyntaxKind::Colon).then(space)
        });
    }
}

/// Nu reads `--` itself as a short flag, and gives a `(-f)` after it to that one.
/// A flag holding a `(` has its short flag already, and nu splits it at the first `(`.
fn is_long_flag(text: &str) -> bool {
    text.starts_with("--") && text.len() > 2 && !text.contains('(')
}
