//! Kinds of tokens and nodes in the CST.

macro_rules! syntax_kinds {
    ($($(#[$meta:meta])* $name:ident,)*) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
        #[repr(u16)]
        pub enum SyntaxKind {
            $($(#[$meta])* $name,)*
        }

        impl SyntaxKind {
            /// Every kind, in discriminant order.
            pub const ALL: &[SyntaxKind] = &[$(SyntaxKind::$name,)*];
        }
    };
}

syntax_kinds! {
    // Trivia.
    /// Spaces, tabs, and a `\r` not followed by `\n`.
    Whitespace,
    /// `\n` or `\r\n`.
    Newline,
    /// `#` up to the end of the line, excluding the line break.
    Comment,

    // Separators.
    Pipe,
    PipePipe,
    Semicolon,
    /// `,` when the mode treats it as a special token.
    Comma,
    /// `:` when the mode treats it as a special token.
    Colon,
    /// `=` when the mode treats it as a special token, as in `[--x=1]`.
    Eq,
    /// `.` when the mode treats it as a special token.
    Dot,
    /// `?` when the mode treats it as a special token.
    Question,
    /// `!` when the mode treats it as a special token.
    Bang,
    LBracket,
    RBracket,
    LBrace,
    RBrace,
    LParen,
    RParen,
    /// `<` of type parameters.
    /// The parser emits this, not the lexer.
    LAngle,
    /// `>` of type parameters.
    /// The parser emits this, not the lexer.
    RAngle,

    // Assignment and redirection.
    /// `=`, `+=`, `++=`, `-=`, `*=`, or `/=` as a whole item.
    Assign,
    /// `o>` or `out>`
    OutGt,
    /// `o>>` or `out>>`
    OutGtGt,
    /// `e>` or `err>`
    ErrGt,
    /// `e>>` or `err>>`
    ErrGtGt,
    /// `o+e>`, `e+o>`, `out+err>`, or `err+out>`
    OutErrGt,
    /// `o+e>>`, `e+o>>`, `out+err>>`, or `err+out>>`
    OutErrGtGt,
    /// `e>|` or `err>|`
    ErrGtPipe,
    /// `o+e>|`, `e+o>|`, `out+err>|`, or `err+out>|`
    OutErrGtPipe,

    // Items.
    /// Exactly one `'...'`, `"..."`, or `` `...` ``.
    String,
    /// Exactly one `r#'...'#`, with any number of `#`.
    RawString,
    /// Exactly one `$"..."` or `$'...'`.
    InterpString,
    /// Any other item, such as `[a b]` or a mixture like `a"b"c`.
    Word,
    /// An item with a lexing error, or a node wrapping a parse error.
    Error,

    // Nodes. Only Root starts or ends with trivia.
    Root,
    /// A statement. Ends before `;`, the end of the file,
    /// or a line break, unless a `|` ends that line or, past any comment lines, starts the next.
    Pipeline,
    /// An element of a `Pipeline`: a `Call` or an `ExternalCall`, then any `Redirection`s.
    /// Nu allows arguments after a redirection; they stay bare tokens.
    /// Starts with a `Redirection` instead, its operator an `Error`, when no command precedes it.
    /// Ends before a pipe (`|`, `e>|`, or `o+e>|`), `;`, a line break, or the end of the file.
    /// A pipe with nothing before or after it, as in `| ls` or `ls | | first`, yields no element.
    PipelineElement,
    /// A command name followed by its arguments.
    Call,
    /// Same as `Call`, but the name starts with `^`, so the command is external.
    ExternalCall,
    /// A redirection to a file: the operator, such as `o>`, and its target.
    /// Has no target only when the operator is an `Error`.
    Redirection,
}

impl SyntaxKind {
    pub fn is_trivia(self) -> bool {
        matches!(
            self,
            SyntaxKind::Whitespace | SyntaxKind::Newline | SyntaxKind::Comment
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
    fn raw_roundtrip() {
        for &kind in SyntaxKind::ALL {
            assert_eq!(SyntaxKind::from_raw(kind.to_raw()), kind);
        }
    }
}
