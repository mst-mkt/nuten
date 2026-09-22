//! Typed views over the nodes of the CST, which rowan calls the AST.
//! One type per node kind, each wrapping a `SyntaxNode` of that kind.
//! Their accessors read the shape the parser gives a node.
//! The `NodeShape` invariant holds every tree to that shape, so the accessors can count on it.

pub use rowan::ast::AstNode;
use rowan::{NodeOrToken, ast::support};

use crate::{NuLanguage, SyntaxElement, SyntaxKind, SyntaxNode, SyntaxToken, kind::syntax_kinds};

/// The node kinds of `syntax_kinds`, with `Error`, which is among its tokens.
macro_rules! nodes {
    (
        tokens { $($tokens:tt)* }
        nodes { $($(#[$meta:meta])* $node:ident,)* }
    ) => {
        typed_nodes! {
            /// A node wrapping a parse error.
            Error,
            $($(#[$meta])* $node,)*
        }
    };
}

macro_rules! typed_nodes {
    ($($(#[$meta:meta])* $name:ident,)*) => {
        $(
            $(#[$meta])*
            #[derive(Debug, Clone, PartialEq, Eq, Hash)]
            pub struct $name(SyntaxNode);

            impl AstNode for $name {
                type Language = NuLanguage;

                fn can_cast(kind: SyntaxKind) -> bool {
                    kind == SyntaxKind::$name
                }

                fn cast(node: SyntaxNode) -> Option<Self> {
                    Self::can_cast(node.kind()).then_some(Self(node))
                }

                fn syntax(&self) -> &SyntaxNode {
                    &self.0
                }
            }
        )*

        /// Any node, told apart by its kind.
        #[derive(Debug, Clone, PartialEq, Eq, Hash)]
        pub enum Node {
            $($name($name),)*
        }

        impl AstNode for Node {
            type Language = NuLanguage;

            fn can_cast(kind: SyntaxKind) -> bool {
                matches!(kind, $(SyntaxKind::$name)|*)
            }

            fn cast(node: SyntaxNode) -> Option<Self> {
                match node.kind() {
                    $(SyntaxKind::$name => Some(Node::$name($name(node))),)*
                    _ => None,
                }
            }

            fn syntax(&self) -> &SyntaxNode {
                match self {
                    $(Node::$name(node) => node.syntax(),)*
                }
            }
        }
    };
}

syntax_kinds!(nodes);

/// A node or a token, the node typed.
pub type Element = NodeOrToken<Node, SyntaxToken>;

pub fn typed(element: SyntaxElement) -> Element {
    match element {
        NodeOrToken::Node(node) => {
            NodeOrToken::Node(Node::cast(node).expect("a node has a node kind"))
        },
        NodeOrToken::Token(token) => NodeOrToken::Token(token),
    }
}

pub fn kind_of(element: &Element) -> SyntaxKind {
    match element {
        NodeOrToken::Node(node) => node.syntax().kind(),
        NodeOrToken::Token(token) => token.kind(),
    }
}

/// A node the parser makes of a word enclosed in brackets.
/// The opening bracket is its first child, or the second after the name of a `Type`.
/// The closing one is its last, whatever comes between.
pub trait Bracketed: AstNode<Language = NuLanguage> {
    fn open(&self) -> SyntaxToken {
        self.syntax()
            .first_child_or_token()
            .and_then(SyntaxElement::into_token)
            .expect("a bracketed node opens with its bracket")
    }

    fn close(&self) -> SyntaxToken {
        self.syntax()
            .last_child_or_token()
            .and_then(SyntaxElement::into_token)
            .expect("a bracketed node closes with its bracket")
    }

    /// What the brackets hold, trivia included.
    fn inside(&self) -> Inside {
        self.following(&self.open().into())
    }

    /// What follows `after`, a child, up to the closing bracket, trivia included.
    fn following(&self, after: &SyntaxElement) -> Inside {
        Inside {
            next: after.next_sibling_or_token(),
            close: self.close(),
        }
    }
}

/// The children of a bracketed node after one of them, up to its closing bracket, in source order.
/// A struct rather than an `impl Iterator`, so that `Bracketed` can be a trait object.
#[derive(Debug, Clone)]
pub struct Inside {
    next: Option<SyntaxElement>,
    close: SyntaxToken,
}

impl Iterator for Inside {
    type Item = SyntaxElement;

    fn next(&mut self) -> Option<SyntaxElement> {
        let element = self
            .next
            .take()
            .filter(|element| element.as_token() != Some(&self.close))?;
        self.next = element.next_sibling_or_token();
        Some(element)
    }
}

/// Implements `Bracketed` for the node kinds the parser makes of a bracketed word.
/// Each takes the methods given in braces, and `Node::brackets` comes from the same table.
macro_rules! bracketed {
    ($($name:ident: $($open:ident $close:ident)|+ $({ $($methods:tt)* })?,)*) => {
        $(impl Bracketed for $name { $($($methods)*)? })*

        impl Node {
            /// The kinds of the brackets a `Bracketed` node may have.
            fn brackets(&self) -> Option<&'static [(SyntaxKind, SyntaxKind)]> {
                match self {
                    $(Node::$name(_) => Some(&[$((SyntaxKind::$open, SyntaxKind::$close)),+]),)*
                    _ => None,
                }
            }
        }
    };
}

bracketed! {
    Subexpression: LParen RParen,
    List: LBracket RBracket,
    Table: LBracket RBracket,
    Signature: LBracket RBracket | LParen RParen,
    Record: LBrace RBrace,
    Block: LBrace RBrace,
    MatchArms: LBrace RBrace,
    Type: LAngle RAngle {
        /// The `<`, right after the name.
        fn open(&self) -> SyntaxToken {
            self.name()
                .next_sibling_or_token()
                .and_then(SyntaxElement::into_token)
                .expect("a type opens its brackets right after its name")
        }
    },
}

impl Node {
    /// What the node lacks of the shape the accessors read, if anything.
    pub fn flaw(&self) -> Option<&'static str> {
        // The inside is read through the brackets, so a missing one is reported first.
        self.bracket_flaw().or_else(|| self.inside_flaw())
    }

    fn bracket_flaw(&self) -> Option<&'static str> {
        let pairs = self.brackets()?;
        let node = self.syntax();
        let first = node.first_child_or_token();
        let opening = match self {
            Node::Type(_) => {
                if first.as_ref().and_then(kind_if_token) != Some(SyntaxKind::Word) {
                    return Some("does not start with its name");
                }
                first.and_then(|name| name.next_sibling_or_token())
            },
            _ => first,
        };
        let Some(close) = opening
            .as_ref()
            .and_then(kind_if_token)
            .and_then(|open| closer(pairs, open))
        else {
            return Some("does not open with its bracket");
        };
        let last = node.last_child_or_token();
        (last.as_ref().and_then(kind_if_token) != Some(close))
            .then_some("does not close with its bracket")
    }

    fn inside_flaw(&self) -> Option<&'static str> {
        match self {
            Node::Block(block) => {
                let params = block.params()?;
                block
                    .inside()
                    .take_while(|element| element.as_node() != Some(params.syntax()))
                    .any(|element| !is_line_trivia(element.kind()))
                    .then_some("does not hold its parameters right after its brace")
            },
            Node::Table(table) => table
                .try_header()
                .is_none()
                .then_some("does not hold its column names and their `;` first"),
            Node::CellPath(_) | Node::Range(_) | Node::Spread(_) => self
                .syntax()
                .children_with_tokens()
                .any(|child| child.kind().is_trivia())
                .then_some("holds trivia among its pieces"),
            _ => None,
        }
    }
}

/// The kind of `element`, when it is a token and not a node.
fn kind_if_token(element: &SyntaxElement) -> Option<SyntaxKind> {
    element.as_token().map(SyntaxToken::kind)
}

/// The closer of the pair `open` starts, among `pairs`.
fn closer(pairs: &[(SyntaxKind, SyntaxKind)], open: SyntaxKind) -> Option<SyntaxKind> {
    pairs
        .iter()
        .find_map(|&(opener, close)| (opener == open).then_some(close))
}

/// Trivia that is not a comment.
fn is_line_trivia(kind: SyntaxKind) -> bool {
    matches!(kind, SyntaxKind::Whitespace | SyntaxKind::Newline)
}

impl Block {
    /// The parameters of a closure, right after the `{`.
    pub fn params(&self) -> Option<ClosureParams> {
        support::child(self.syntax())
    }

    /// What follows the parameters, or the `{`.
    /// The statements, the `;` and the comments among them, and the trivia around them.
    pub fn statements(&self) -> Inside {
        match self.params() {
            Some(params) => self.following(&params.syntax().clone().into()),
            None => self.inside(),
        }
    }
}

impl Table {
    fn try_header(&self) -> Option<(SyntaxElement, SyntaxToken)> {
        let mut items = self.inside().filter(|item| !is_line_trivia(item.kind()));
        let columns = items.next()?;
        let semicolon = items
            .next()?
            .into_token()
            .filter(|token| token.kind() == SyntaxKind::Semicolon)?;
        Some((columns, semicolon))
    }

    /// The column names, a `List` or a `Word`, and the `;` after them, past any trivia.
    /// The rows are what follows the `;`.
    pub fn header(&self) -> (Element, SyntaxToken) {
        let (columns, semicolon) = self
            .try_header()
            .expect("a table holds its column names and their `;` first");
        (typed(columns), semicolon)
    }
}

impl Def {
    /// The body, its last child when that is a block; an `extern`, or a `def` cut short, has none.
    pub fn body(&self) -> Option<Block> {
        self.syntax().last_child().and_then(Block::cast)
    }
}

impl Type {
    /// The name before the `<`; the parameters or fields are what the brackets hold.
    pub fn name(&self) -> SyntaxToken {
        self.syntax()
            .first_child_or_token()
            .and_then(SyntaxElement::into_token)
            .expect("a type starts with its name")
    }
}

#[cfg(test)]
mod tests {
    use std::iter;

    use rowan::{GreenNodeBuilder, NodeOrToken};

    use super::{
        AstNode, Block, Bracketed, Def, List, Node, Pipeline, Table, Type, kind_of, typed,
    };
    use crate::{SyntaxElement, SyntaxKind, SyntaxNode};

    /// A tree written down: a node of a kind with what it holds, or a token.
    enum Tree {
        Node(SyntaxKind, Vec<Tree>),
        Token(SyntaxKind, &'static str),
    }

    fn node(kind: SyntaxKind, children: Vec<Tree>) -> Tree {
        Tree::Node(kind, children)
    }

    fn token(kind: SyntaxKind, text: &'static str) -> Tree {
        Tree::Token(kind, text)
    }

    fn build(tree: &Tree) -> SyntaxNode {
        fn add(builder: &mut GreenNodeBuilder<'_>, tree: &Tree) {
            match tree {
                Tree::Node(kind, children) => {
                    builder.start_node(kind.to_raw());
                    for child in children {
                        add(builder, child);
                    }
                    builder.finish_node();
                },
                Tree::Token(kind, text) => builder.token(kind.to_raw(), text),
            }
        }
        let mut builder = GreenNodeBuilder::new();
        add(&mut builder, tree);
        SyntaxNode::new_root(builder.finish())
    }

    fn node_of(kind: SyntaxKind) -> SyntaxNode {
        build(&node(kind, vec![]))
    }

    /// `inside` between `[` and `]`.
    fn bracketed(inside: Vec<Tree>) -> Vec<Tree> {
        iter::once(token(SyntaxKind::LBracket, "["))
            .chain(inside)
            .chain(iter::once(token(SyntaxKind::RBracket, "]")))
            .collect()
    }

    fn kinds(elements: impl Iterator<Item = SyntaxElement>) -> Vec<SyntaxKind> {
        elements.map(|element| element.kind()).collect()
    }

    #[test]
    fn node_casts_error_and_every_kind_from_root_on() {
        for &kind in SyntaxKind::ALL {
            let is_node = kind >= SyntaxKind::Root || kind == SyntaxKind::Error;
            assert_eq!(Node::can_cast(kind), is_node, "{kind:?}");
            let cast = Node::cast(node_of(kind)).map(|node| node.syntax().kind());
            assert_eq!(cast, is_node.then_some(kind), "{kind:?}");
        }
    }

    #[test]
    fn typed_node_casts_its_kind_only() {
        let block = node_of(SyntaxKind::Block);
        assert!(Block::cast(block.clone()).is_some());
        assert!(Pipeline::cast(block.clone()).is_none());
        assert!(matches!(Node::cast(block), Some(Node::Block(_))));
    }

    #[test]
    fn typed_types_node_and_leaves_token() {
        let root = build(&node(
            SyntaxKind::Root,
            vec![
                node(SyntaxKind::Pipeline, vec![token(SyntaxKind::Word, "ls")]),
                token(SyntaxKind::Newline, "\n"),
            ],
        ));
        let elements: Vec<_> = root.children_with_tokens().map(typed).collect();
        assert!(matches!(elements[0], NodeOrToken::Node(Node::Pipeline(_))));
        assert!(
            matches!(&elements[1], NodeOrToken::Token(token) if token.kind() == SyntaxKind::Newline)
        );
        assert_eq!(kind_of(&elements[0]), SyntaxKind::Pipeline);
        assert_eq!(kind_of(&elements[1]), SyntaxKind::Newline);
    }

    #[test]
    fn bracketed_node_gives_its_brackets_and_what_they_hold() {
        let list = List::cast(build(&node(
            SyntaxKind::List,
            bracketed(vec![
                token(SyntaxKind::Whitespace, " "),
                token(SyntaxKind::Word, "a"),
                token(SyntaxKind::Newline, "\n"),
            ]),
        )))
        .unwrap();
        assert_eq!(list.open().text(), "[");
        assert_eq!(list.close().text(), "]");
        assert_eq!(
            kinds(list.inside()),
            [
                SyntaxKind::Whitespace,
                SyntaxKind::Word,
                SyntaxKind::Newline
            ]
        );
    }

    #[test]
    fn empty_brackets_hold_nothing() {
        let list = List::cast(build(&node(SyntaxKind::List, bracketed(vec![])))).unwrap();
        assert_eq!(kinds(list.inside()), []);
    }

    #[test]
    fn statements_of_block_follow_its_params_or_its_brace() {
        let with_params = node(
            SyntaxKind::Block,
            vec![
                token(SyntaxKind::LBrace, "{"),
                token(SyntaxKind::Whitespace, " "),
                node(
                    SyntaxKind::ClosureParams,
                    vec![token(SyntaxKind::Pipe, "|"), token(SyntaxKind::Pipe, "|")],
                ),
                token(SyntaxKind::Newline, "\n"),
                token(SyntaxKind::Word, "ls"),
                token(SyntaxKind::RBrace, "}"),
            ],
        );
        let without = node(
            SyntaxKind::Block,
            vec![
                token(SyntaxKind::LBrace, "{"),
                token(SyntaxKind::Newline, "\n"),
                token(SyntaxKind::Word, "ls"),
                token(SyntaxKind::RBrace, "}"),
            ],
        );
        for (tree, has_params) in [(with_params, true), (without, false)] {
            let block = Block::cast(build(&tree)).unwrap();
            assert_eq!(block.params().is_some(), has_params);
            assert_eq!(
                kinds(block.statements()),
                [SyntaxKind::Newline, SyntaxKind::Word]
            );
        }
    }

    #[test]
    fn table_gives_its_columns_and_semicolon_with_rows_following() {
        let table = Table::cast(build(&node(
            SyntaxKind::Table,
            bracketed(vec![
                token(SyntaxKind::Newline, "\n"),
                node(
                    SyntaxKind::List,
                    bracketed(vec![token(SyntaxKind::Word, "a")]),
                ),
                token(SyntaxKind::Whitespace, " "),
                token(SyntaxKind::Semicolon, ";"),
                token(SyntaxKind::Whitespace, " "),
                token(SyntaxKind::Word, "[1]"),
            ]),
        )))
        .unwrap();
        let (columns, semicolon) = table.header();
        assert!(matches!(columns, NodeOrToken::Node(Node::List(_))));
        assert_eq!(semicolon.text(), ";");
        assert_eq!(
            kinds(table.following(&semicolon.into())),
            [SyntaxKind::Whitespace, SyntaxKind::Word]
        );
    }

    #[test]
    fn type_gives_its_name_and_what_its_brackets_hold() {
        let type_ = Type::cast(build(&node(
            SyntaxKind::Type,
            vec![
                token(SyntaxKind::Word, "list"),
                token(SyntaxKind::LAngle, "<"),
                token(SyntaxKind::Word, "int"),
                token(SyntaxKind::Comma, ","),
                token(SyntaxKind::Whitespace, " "),
                token(SyntaxKind::Word, "string"),
                token(SyntaxKind::RAngle, ">"),
            ],
        )))
        .unwrap();
        assert_eq!(type_.name().text(), "list");
        assert_eq!(type_.open().text(), "<");
        assert_eq!(type_.close().text(), ">");
        assert_eq!(
            kinds(type_.inside()),
            [
                SyntaxKind::Word,
                SyntaxKind::Comma,
                SyntaxKind::Whitespace,
                SyntaxKind::Word
            ]
        );
    }

    #[test]
    fn def_body_is_its_last_child_when_that_is_block() {
        let block = || {
            node(
                SyntaxKind::Block,
                vec![
                    token(SyntaxKind::LBrace, "{"),
                    token(SyntaxKind::RBrace, "}"),
                ],
            )
        };
        let signature = || node(SyntaxKind::Signature, bracketed(vec![]));
        for (children, has_body) in [
            (
                vec![
                    token(SyntaxKind::Word, "def"),
                    token(SyntaxKind::Word, "f"),
                    signature(),
                    block(),
                ],
                true,
            ),
            (
                vec![
                    token(SyntaxKind::Word, "extern"),
                    token(SyntaxKind::Word, "f"),
                    signature(),
                ],
                false,
            ),
            (
                vec![
                    token(SyntaxKind::Word, "def"),
                    token(SyntaxKind::Word, "f"),
                    signature(),
                    token(SyntaxKind::Word, "{}"),
                ],
                false,
            ),
            (
                vec![
                    token(SyntaxKind::Word, "def"),
                    token(SyntaxKind::Word, "f"),
                    block(),
                    signature(),
                ],
                false,
            ),
        ] {
            let def = Def::cast(build(&node(SyntaxKind::Def, children))).unwrap();
            assert_eq!(def.body().is_some(), has_body);
        }
    }

    #[test]
    fn flaw_names_what_node_lacks() {
        let params = || {
            node(
                SyntaxKind::ClosureParams,
                vec![token(SyntaxKind::Pipe, "|"), token(SyntaxKind::Pipe, "|")],
            )
        };
        for (tree, flaw) in [
            (
                node(SyntaxKind::Block, vec![]),
                Some("does not open with its bracket"),
            ),
            (
                node(SyntaxKind::Block, vec![token(SyntaxKind::Word, "a")]),
                Some("does not open with its bracket"),
            ),
            (
                node(
                    SyntaxKind::Block,
                    vec![token(SyntaxKind::LBrace, "{"), token(SyntaxKind::Word, "a")],
                ),
                Some("does not close with its bracket"),
            ),
            (
                node(
                    SyntaxKind::Block,
                    vec![
                        token(SyntaxKind::LBrace, "{"),
                        node(SyntaxKind::Record, vec![token(SyntaxKind::RBrace, "}")]),
                    ],
                ),
                Some("does not close with its bracket"),
            ),
            (
                node(
                    SyntaxKind::Block,
                    vec![
                        token(SyntaxKind::LBrace, "{"),
                        token(SyntaxKind::Word, "a"),
                        params(),
                        token(SyntaxKind::RBrace, "}"),
                    ],
                ),
                Some("does not hold its parameters right after its brace"),
            ),
            (
                node(
                    SyntaxKind::Block,
                    vec![
                        token(SyntaxKind::LBrace, "{"),
                        token(SyntaxKind::Newline, "\n"),
                        params(),
                        token(SyntaxKind::Word, "a"),
                        token(SyntaxKind::RBrace, "}"),
                    ],
                ),
                None,
            ),
            (
                node(SyntaxKind::Table, bracketed(vec![])),
                Some("does not hold its column names and their `;` first"),
            ),
            (
                node(
                    SyntaxKind::Table,
                    bracketed(vec![token(SyntaxKind::Word, "[a]")]),
                ),
                Some("does not hold its column names and their `;` first"),
            ),
            (
                node(
                    SyntaxKind::Table,
                    bracketed(vec![
                        node(
                            SyntaxKind::List,
                            bracketed(vec![token(SyntaxKind::Word, "a")]),
                        ),
                        node(
                            SyntaxKind::List,
                            bracketed(vec![token(SyntaxKind::Word, "1")]),
                        ),
                    ]),
                ),
                Some("does not hold its column names and their `;` first"),
            ),
            (
                node(
                    SyntaxKind::Table,
                    bracketed(vec![
                        token(SyntaxKind::Word, "[a]"),
                        token(SyntaxKind::Comment, "# c"),
                        token(SyntaxKind::Semicolon, ";"),
                    ]),
                ),
                Some("does not hold its column names and their `;` first"),
            ),
            (
                node(
                    SyntaxKind::Table,
                    bracketed(vec![
                        token(SyntaxKind::Word, "[a]"),
                        token(SyntaxKind::Semicolon, ";"),
                    ]),
                ),
                None,
            ),
            (node(SyntaxKind::List, bracketed(vec![])), None),
            (
                node(
                    SyntaxKind::List,
                    vec![
                        token(SyntaxKind::LParen, "("),
                        token(SyntaxKind::RParen, ")"),
                    ],
                ),
                Some("does not open with its bracket"),
            ),
            (node(SyntaxKind::Signature, bracketed(vec![])), None),
            (
                node(
                    SyntaxKind::Signature,
                    vec![
                        token(SyntaxKind::LParen, "("),
                        token(SyntaxKind::RParen, ")"),
                    ],
                ),
                None,
            ),
            (
                node(
                    SyntaxKind::Signature,
                    vec![
                        token(SyntaxKind::LBrace, "{"),
                        token(SyntaxKind::RBrace, "}"),
                    ],
                ),
                Some("does not open with its bracket"),
            ),
            (
                node(
                    SyntaxKind::Signature,
                    vec![
                        token(SyntaxKind::LParen, "("),
                        token(SyntaxKind::RBracket, "]"),
                    ],
                ),
                Some("does not close with its bracket"),
            ),
            (
                node(
                    SyntaxKind::Signature,
                    vec![
                        token(SyntaxKind::LBracket, "["),
                        token(SyntaxKind::RParen, ")"),
                    ],
                ),
                Some("does not close with its bracket"),
            ),
            (
                node(SyntaxKind::Type, vec![token(SyntaxKind::LAngle, "<")]),
                Some("does not start with its name"),
            ),
            (
                node(
                    SyntaxKind::Type,
                    vec![node(SyntaxKind::List, bracketed(vec![]))],
                ),
                Some("does not start with its name"),
            ),
            (
                node(SyntaxKind::Type, vec![token(SyntaxKind::Word, "list")]),
                Some("does not open with its bracket"),
            ),
            (
                node(
                    SyntaxKind::Type,
                    vec![
                        token(SyntaxKind::Word, "list"),
                        token(SyntaxKind::Word, "int"),
                        token(SyntaxKind::RAngle, ">"),
                    ],
                ),
                Some("does not open with its bracket"),
            ),
            (
                node(
                    SyntaxKind::Type,
                    vec![
                        token(SyntaxKind::Word, "list"),
                        token(SyntaxKind::LAngle, "<"),
                        token(SyntaxKind::Word, "int"),
                    ],
                ),
                Some("does not close with its bracket"),
            ),
            (
                node(
                    SyntaxKind::Type,
                    vec![
                        token(SyntaxKind::Word, "list"),
                        token(SyntaxKind::LAngle, "<"),
                        token(SyntaxKind::RAngle, ">"),
                    ],
                ),
                None,
            ),
            (
                node(
                    SyntaxKind::CellPath,
                    vec![
                        token(SyntaxKind::Word, "$x"),
                        token(SyntaxKind::Dot, "."),
                        token(SyntaxKind::Comment, "#a"),
                    ],
                ),
                Some("holds trivia among its pieces"),
            ),
            (
                node(
                    SyntaxKind::CellPath,
                    vec![
                        token(SyntaxKind::Word, "$x"),
                        token(SyntaxKind::Whitespace, " "),
                        token(SyntaxKind::Dot, "."),
                    ],
                ),
                Some("holds trivia among its pieces"),
            ),
            (
                node(
                    SyntaxKind::CellPath,
                    vec![
                        token(SyntaxKind::Word, "$x"),
                        token(SyntaxKind::Dot, "."),
                        token(SyntaxKind::Word, "a"),
                    ],
                ),
                None,
            ),
            (
                node(
                    SyntaxKind::Range,
                    vec![
                        token(SyntaxKind::Word, "1"),
                        token(SyntaxKind::Newline, "\n"),
                        token(SyntaxKind::RangeOperator, ".."),
                    ],
                ),
                Some("holds trivia among its pieces"),
            ),
            (
                node(
                    SyntaxKind::Range,
                    vec![
                        token(SyntaxKind::Word, "1"),
                        token(SyntaxKind::RangeOperator, ".."),
                        token(SyntaxKind::Word, "2"),
                    ],
                ),
                None,
            ),
            (
                node(
                    SyntaxKind::Spread,
                    vec![
                        token(SyntaxKind::SpreadOperator, "..."),
                        token(SyntaxKind::Whitespace, " "),
                        token(SyntaxKind::Word, "$x"),
                    ],
                ),
                Some("holds trivia among its pieces"),
            ),
            (
                node(SyntaxKind::Pipeline, vec![token(SyntaxKind::Word, "a")]),
                None,
            ),
        ] {
            let node = Node::cast(build(&tree)).unwrap();
            assert_eq!(node.flaw(), flaw, "{node:?}");
        }
    }
}
