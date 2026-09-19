use nuten_doc::{Element, space, text};
use nuten_syntax::{
    NodeOrToken, SyntaxKind,
    ast::{self, AstNode, Param, Type, TypeField},
};

use super::{Child, Formatter, children, separator, without_commas};

impl<'a> Formatter<'a> {
    /// One parameter of a signature: its name, then its `(-f)` joined to it
    /// where nu reads the two as one flag, its `:` joined too unless a
    /// comment comes before it, its type, and its `=` and default value, each
    /// one space apart. A `,` is dropped.
    pub(super) fn param(&mut self, param: &Param) -> Vec<Element<'a>> {
        let mut out = Vec::new();
        let mut prev: Option<ast::Element> = None;
        for child in without_commas(children(param.syntax().children_with_tokens())) {
            let Child {
                element,
                newlines,
            } = child;
            let kind = ast::kind_of(&element);
            match &element {
                NodeOrToken::Token(child) if kind == SyntaxKind::Comment => {
                    out.extend(self.comment(child, newlines, false));
                },
                _ => {
                    let joined = match &prev {
                        None => true,
                        // A `:` after a comment stays on the next line: joined,
                        // it would land inside the comment or, as a line
                        // suffix, the comment would land after the `]`, and nu
                        // would lose the description.
                        // A `ShortFlag` joins the name it follows, a long flag;
                        // joined to a type or a default value, it would become
                        // part of that.
                        Some(prev) => {
                            (kind == SyntaxKind::Colon && ast::kind_of(prev) != SyntaxKind::Comment)
                                || (kind == SyntaxKind::ShortFlag
                                    && prev.as_token() == param.syntax().first_token().as_ref()
                                    && prev
                                        .as_token()
                                        .is_some_and(|name| is_long_flag(name.text())))
                        },
                    };
                    if !joined {
                        out.extend(separator(prev.as_ref().map(ast::kind_of), space));
                    }
                    out.extend(self.element(&element));
                },
            }
            prev = Some(element);
        }
        out
    }

    /// A type that takes parameters, on one line: its name, `<`, its
    /// parameters or fields `, ` apart, and `>`, with no space inside the
    /// brackets. A `,` is dropped, and one is printed between two parameters
    /// whether the source had one or not, which the token invariant cannot
    /// see and the nu oracle does. No comment comes inside, since the parser
    /// keeps a type holding one as one token.
    pub(super) fn type_(&mut self, type_: &Type) -> Vec<Element<'a>> {
        let mut out = Vec::new();
        // Whether the `<` has been printed, and whether no parameter has
        // since.
        let mut open = false;
        let mut first = true;
        for child in without_commas(children(type_.syntax().children_with_tokens())) {
            let kind = ast::kind_of(&child.element);
            // The name and the brackets touch what is around them; every
            // parameter but the first follows a `, `.
            if open && kind != SyntaxKind::RAngle {
                if !first {
                    out.push(text(","));
                    out.push(space());
                }
                first = false;
            }
            if kind == SyntaxKind::LAngle {
                open = true;
            }
            out.extend(self.element(&child.element));
        }
        out
    }

    /// One field of a type: its key, its `:` right after it, and its type one
    /// space after that. A `,` here is the type nu takes for the key, so it
    /// stays, unlike one among the fields.
    pub(super) fn type_field(&mut self, field: &TypeField) -> Vec<Element<'a>> {
        let mut out = Vec::new();
        for child in children(field.syntax().children_with_tokens()) {
            if !out.is_empty() && ast::kind_of(&child.element) != SyntaxKind::Colon {
                out.push(space());
            }
            out.extend(self.element(&child.element));
        }
        out
    }
}

/// Whether `text` is a long flag that takes a `(-f)`. Nu reads `--` itself
/// as a short flag, and gives a `(-f)` after it to that one; a flag holding
/// a `(` has its short flag already, and nu splits it at the first `(`.
fn is_long_flag(text: &str) -> bool {
    text.starts_with("--") && text.len() > 2 && !text.contains('(')
}
