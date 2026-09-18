use nuten_doc::{Element, space, text};
use nuten_syntax::{SyntaxElement, SyntaxKind, SyntaxNode};

use super::{Child, children, comment, node, separator, token};

/// One parameter of a signature: its name, then its `(-f)` joined to it where
/// nu reads the two as one flag, its `:` joined too unless a comment comes
/// before it, its type, and its `=` and default value, each one space apart.
/// A `,` is dropped.
pub(super) fn param<'a>(src: &'a str, param: &SyntaxNode) -> Vec<Element<'a>> {
    let mut out = Vec::new();
    let mut prev: Option<SyntaxElement> = None;
    for child in children(param) {
        let Child {
            element,
            newlines,
        } = child;
        let kind = element.kind();
        if kind == SyntaxKind::Comma {
            continue;
        }
        if kind == SyntaxKind::Comment {
            let SyntaxElement::Token(child) = &element else {
                unreachable!("a comment is a token");
            };
            out.extend(comment(src, child, newlines, false));
            prev = Some(element);
            continue;
        }
        let joined = match &prev {
            None => true,
            // A `:` after a comment stays on the next line: joined, it would
            // land inside the comment or, as a line suffix, the comment would
            // land after the `]`, and nu would lose the description.
            // A `ShortFlag` joins the name it follows, a long flag; joined
            // to a type or a default value, it would become part of that.
            Some(prev) => {
                (kind == SyntaxKind::Colon && prev.kind() != SyntaxKind::Comment)
                    || (kind == SyntaxKind::ShortFlag
                        && prev.as_token() == param.first_token().as_ref()
                        && prev
                            .as_token()
                            .is_some_and(|name| is_long_flag(name.text())))
            },
        };
        if !joined {
            out.extend(separator(prev.as_ref().map(SyntaxElement::kind), space));
        }
        match &element {
            SyntaxElement::Node(child) => out.extend(node(src, child)),
            SyntaxElement::Token(child) => out.push(token(src, child)),
        }
        prev = Some(element);
    }
    out
}

/// Whether `text` is a long flag that takes a `(-f)`. Nu reads `--` itself
/// as a short flag, and gives a `(-f)` after it to that one; a flag holding
/// a `(` has its short flag already, and nu splits it at the first `(`.
fn is_long_flag(text: &str) -> bool {
    text.starts_with("--") && text.len() > 2 && !text.contains('(')
}

/// A type that takes parameters, on one line: its name, `<`, its parameters
/// or fields `, ` apart, and `>`, with no space inside the brackets. A `,` is
/// dropped, and one is printed between two parameters whether the source had
/// one or not, which the token invariant cannot see and the nu oracle does.
/// No comment comes inside, since the parser keeps a type holding one as one
/// token.
pub(super) fn type_<'a>(src: &'a str, type_: &SyntaxNode) -> Vec<Element<'a>> {
    let mut out = Vec::new();
    // Whether the `<` has been printed, and whether no parameter has since.
    let mut open = false;
    let mut first = true;
    for child in children(type_) {
        let kind = child.element.kind();
        if kind == SyntaxKind::Comma {
            continue;
        }
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
        match &child.element {
            SyntaxElement::Node(child) => out.extend(node(src, child)),
            SyntaxElement::Token(child) => out.push(token(src, child)),
        }
    }
    out
}

/// One field of a type: its key, its `:` right after it, and its type one
/// space after that. A `,` here is the type nu takes for the key, so it
/// stays, unlike one among the fields.
pub(super) fn type_field<'a>(src: &'a str, field: &SyntaxNode) -> Vec<Element<'a>> {
    let mut out = Vec::new();
    for child in children(field) {
        if !out.is_empty() && child.element.kind() != SyntaxKind::Colon {
            out.push(space());
        }
        match &child.element {
            SyntaxElement::Node(child) => out.extend(node(src, child)),
            SyntaxElement::Token(child) => out.push(token(src, child)),
        }
    }
    out
}
