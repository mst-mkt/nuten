//! Lossless CST for Nushell.

mod kind;

pub use kind::SyntaxKind;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum NuLanguage {}

impl rowan::Language for NuLanguage {
    type Kind = SyntaxKind;

    fn kind_from_raw(raw: rowan::SyntaxKind) -> Self::Kind {
        SyntaxKind::from_raw(raw)
    }

    fn kind_to_raw(kind: Self::Kind) -> rowan::SyntaxKind {
        kind.to_raw()
    }
}

pub type SyntaxNode = rowan::SyntaxNode<NuLanguage>;
pub type SyntaxToken = rowan::SyntaxToken<NuLanguage>;
pub type SyntaxElement = rowan::SyntaxElement<NuLanguage>;

#[cfg(test)]
mod tests {
    use rowan::GreenNodeBuilder;

    use super::{SyntaxKind, SyntaxNode};

    #[test]
    fn build_and_read_back() {
        let mut builder = GreenNodeBuilder::new();
        builder.start_node(SyntaxKind::Root.into());
        builder.token(SyntaxKind::Word.into(), "ls");
        builder.token(SyntaxKind::Whitespace.into(), " ");
        builder.token(SyntaxKind::Pipe.into(), "|");
        builder.token(SyntaxKind::Newline.into(), "\n");
        builder.finish_node();

        let root = SyntaxNode::new_root(builder.finish());
        assert_eq!(root.kind(), SyntaxKind::Root);
        assert_eq!(root.text(), "ls |\n");

        let kinds: Vec<_> = root
            .children_with_tokens()
            .map(|element| element.kind())
            .collect();
        assert_eq!(
            kinds,
            [
                SyntaxKind::Word,
                SyntaxKind::Whitespace,
                SyntaxKind::Pipe,
                SyntaxKind::Newline
            ]
        );
    }
}
