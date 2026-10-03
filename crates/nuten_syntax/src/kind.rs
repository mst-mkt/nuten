/// `SyntaxKind` and `ast` are both made from this one list, so neither can miss a node kind.
/// `Error` is among the tokens, and is a node too.
macro_rules! syntax_kinds {
    ($callback:ident) => {
        $callback! {
            tokens {
                Whitespace,
                Newline,
                Comment,

                Pipe,
                PipePipe,
                Semicolon,
                Comma,
                Colon,
                Eq,
                Dot,
                Question,
                Bang,
                LBracket,
                RBracket,
                LBrace,
                RBrace,
                LParen,
                RParen,
                LAngle,
                RAngle,
                FatArrow,

                Assign,
                OutGt,
                OutGtGt,
                ErrGt,
                ErrGtGt,
                OutErrGt,
                OutErrGtGt,
                ErrGtPipe,
                OutErrGtPipe,
                ListPipe,

                Operator,
                RangeOperator,
                SpreadOperator,

                String,
                RawString,
                InterpString,
                Word,
                ShortFlag,
                InterpText,
                Error,
            }
            nodes {
                Root,
                Pipeline,
                PipelineElement,
                Call,
                /// Nu runs `else` and `catch` as external commands outside an `if` or a `try`.
                /// That is, unless a block defines them or an import before them may bring them.
                ExternalCall,
                CommandName,
                MathExpression,

                If,
                For,
                While,
                Loop,
                Try,
                Match,
                MatchArms,
                MatchArm,
                ListPattern,
                RecordPattern,
                Redirection,
                Assignment,
                Let,
                /// Nu reads what follows `=` as one call to the statement's end.
                /// Nu declares no alias where a value heads what follows `=`.
                Alias,
                Subexpression,
                CellPath,
                Range,
                Spread,
                Interpolation,
                Binary,
                List,
                /// Only where nu takes a value or a block, since nu reads no table elsewhere.
                Table,
                Record,
                RecordField,
                Block,
                ClosureParams,
                Attribute,
                AttributeBlock,
                Def,
                Signature,
                Param,
                Type,
                TypeField,
                IoTypes,
                IoType,
                /// Its body is a `Block` of statements, since nu reads no parameters there.
                Module,
                ExportEnv,
                Use,
                /// A lone `{ }` after the flags is a `Block`, since nu reads it as a closure.
                Where,
            }
        }
    };
}
pub(crate) use syntax_kinds;

macro_rules! kind_enum {
    (
        tokens { $($(#[$token_meta:meta])* $token:ident,)* }
        nodes { $($(#[$node_meta:meta])* $node:ident,)* }
    ) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
        #[repr(u16)]
        pub enum SyntaxKind {
            $($(#[$token_meta])* $token,)*
            $($(#[$node_meta])* $node,)*
        }

        impl SyntaxKind {
            /// Every kind, in discriminant order.
            pub const ALL: &[SyntaxKind] = &[$(SyntaxKind::$token,)* $(SyntaxKind::$node,)*];
        }
    };
}
syntax_kinds!(kind_enum);

impl SyntaxKind {
    pub fn is_trivia(self) -> bool {
        matches!(
            self,
            SyntaxKind::Whitespace | SyntaxKind::Newline | SyntaxKind::Comment
        )
    }

    pub fn is_separator(self) -> bool {
        matches!(self, SyntaxKind::Comma | SyntaxKind::ListPipe)
    }

    /// An `Error` counts, since it is an item the lexer could not read.
    /// A `ShortFlag` is not, since nu reads it as part of the item before it.
    pub fn is_item(self) -> bool {
        matches!(
            self,
            SyntaxKind::Word
                | SyntaxKind::String
                | SyntaxKind::RawString
                | SyntaxKind::InterpString
                | SyntaxKind::Error
        )
    }

    pub fn from_raw(raw: rowan::SyntaxKind) -> SyntaxKind {
        SyntaxKind::ALL[usize::from(raw.0)]
    }

    pub fn to_raw(self) -> rowan::SyntaxKind {
        rowan::SyntaxKind(self as u16)
    }
}

#[cfg(test)]
mod tests {
    use super::SyntaxKind;

    #[test]
    fn every_kind_roundtrips_through_raw() {
        for &kind in SyntaxKind::ALL {
            assert_eq!(SyntaxKind::from_raw(kind.to_raw()), kind);
        }
    }
}
