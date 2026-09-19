//! Typed views over the nodes of the CST, which rowan calls the AST: one type
//! per node kind, each wrapping a `SyntaxNode` of that kind.

pub use rowan::ast::AstNode;

use crate::{NuLanguage, SyntaxKind, SyntaxNode, kind::syntax_kinds};

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

#[cfg(test)]
mod tests {
    use rowan::GreenNodeBuilder;

    use super::{AstNode, Block, Node, Pipeline};
    use crate::{SyntaxKind, SyntaxNode};

    fn node_of(kind: SyntaxKind) -> SyntaxNode {
        let mut builder = GreenNodeBuilder::new();
        builder.start_node(kind.to_raw());
        builder.finish_node();
        SyntaxNode::new_root(builder.finish())
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
}
