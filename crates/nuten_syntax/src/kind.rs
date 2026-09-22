//! Kinds of tokens and nodes in the CST.

/// Calls `$callback` with every kind as `tokens { .. } nodes { .. }`.
/// `SyntaxKind` and `ast` are both made from this one list, so neither can miss a node kind.
/// `Error` is among the tokens, and is a node too.
macro_rules! syntax_kinds {
    ($callback:ident) => {
        $callback! {
            tokens {
                // Trivia.
                /// Spaces, tabs, and a `\r` not followed by `\n`.
                Whitespace,
                /// `\n` or `\r\n`.
                Newline,
                /// `#` up to the end of the line, excluding the line break.
                Comment,

                // Separators.
                // The brackets, the angles, and `=>` come from the parser, not the lexer.
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
                LAngle,
                /// `>` of type parameters.
                RAngle,
                /// `=>` between the pattern of a `match` arm and what the arm does.
                FatArrow,

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
                /// Any other item, such as `ls` or a mixture like `a"b"c`.
                Word,
                /// A `(-f)` of one character, which nu takes for the parameter before it.
                /// The parser emits this, not the lexer.
                ShortFlag,
                /// An item with a lexing error, or a node wrapping a parse error.
                Error,
            }
            nodes {
                /// The whole file: its statements, with the `;` and comments among them.
                /// The only node that starts or ends with trivia.
                Root,
                /// A statement.
                Pipeline,
                /// An element of a `Pipeline`: a `Call` or `ExternalCall`, then any `Redirection`s.
                /// Where nu reads an expression, its items stand bare in place of the call.
                /// It holds an `Assignment` or a `Let` instead, which runs to the statement's end.
                /// Nu allows arguments after a redirection; they stay bare tokens.
                PipelineElement,
                /// A command name followed by its arguments.
                Call,
                /// A `Call` whose name starts with `^`, or is `else` or `catch`.
                /// Nu runs those two as external commands outside an `if` or a `try`.
                ExternalCall,

                // Keyword syntax, read where nu reads the call as the syntax over a `Call`.
                // Each is what a `Call` would be for a command of that name.
                // A body is an argument that starts with `{`.
                /// `if`, the condition, the body, then any number of `else`, `if`, condition, body.
                /// A last `else` takes one more argument.
                /// The `if`s after the `else`s are in this one flat node.
                If,
                /// `for`, the variable, `in`, the value, and the body.
                /// A variable ending in `:` takes its type after it.
                /// The value is at least one argument.
                For,
                /// `while`, at least one argument of the condition, and the body.
                While,
                /// `loop` and the body.
                Loop,
                /// `try` and the body, then optionally `catch` and the body it takes.
                Try,
                /// `match`, at least one argument of the subject, and the arms.
                /// Nu reads the subject as any value.
                Match,
                /// The `{ }` of a `match` read as arms.
                /// `{`, the `MatchArm`s with the `,` between them, and `}`.
                MatchArms,
                /// One arm: the tokens of its pattern and of any `if` guard, `|` among them.
                /// Then `=>` and the one item nu takes as what the arm does.
                MatchArm,
                /// A redirection to a file: the operator, such as `o>`, and its target.
                /// Has no target only when the operator is an `Error`.
                Redirection,
                /// What a `PipelineElement` holds when an assignment operator comes before its end.
                /// The target of a file redirection is no such operator.
                /// What comes before the operator is held by a `PipelineElement` of its own.
                /// Then the operator, with any comments and line breaks before a line-start `|`.
                /// Then any `Pipeline`s of the rest, with the comments and line breaks among them.
                Assignment,
                /// As an `Assignment`, for an element starting with `let`, `mut`, or `const`.
                /// That word, not a `Call`, comes before the arguments.
                Let,
                /// A `Word` in `( )` read as a subexpression: `(`, the statements, and `)`.
                /// Nu reads one wherever it takes a value, a `def` signature aside.
                Subexpression,
                /// A `Word` in `[ ]` read as a list: `[`, what it holds, and `]`.
                /// Nu reads one where it takes a value or a block.
                /// It reads one as an argument of an external command too.
                List,
                /// A `Word` in `[ ]` read as a table where nu takes a value or a block.
                /// `[`, the column names, their `;`, the rows, any `,` among them, and `]`.
                /// The names and each row are a `List`.
                /// One holding a `;` stays a `Word`, since nu reads no table there.
                Table,
                /// A `Word` in `{ }` read as a record: `{`, its fields and spreads, and `}`.
                /// Nu reads one where it takes a value, or a block unless a spread comes first.
                /// It is a `Block` as the body of a `def`.
                Record,
                /// A key, the token nu takes as its `:`, then its value.
                /// Each of those while the record lasts.
                /// A value of several touching tokens is one `Word`.
                RecordField,
                /// A `Word` in `{ }` that is no record: `{`, any parameters, the statements, `}`.
                /// Nu reads one where it takes a value or a block, or an argument of a nu command.
                /// An external command takes it as a string instead.
                /// The body of a `def` is one whatever it holds.
                Block,
                /// The parameters of a closure as unchecked tokens: `|` to the next `|`, or `||`.
                ClosureParams,
                /// A `def` or an `extern` that nu reads as one.
                /// `export` if there, the keyword, any flags, the name, and a `Signature`.
                /// Then the `:` and the items of the input and output types if there.
                /// Then the body, which only a `def` has.
                Def,
                /// The parameters of a `def` or an `extern`.
                /// The `[` or `(`, what it holds, and the matching `]` or `)`.
                Signature,
                /// One parameter of a `Signature`, starting with its name.
                /// Then, in source order, the `ShortFlag`, the `:` and type, the `=` and default.
                /// Each of those while nu still takes it for that parameter.
                /// It holds the `,` that ends it, and the `|`, `;`, and redirections nu ignores.
                Param,
                /// A `Signature` type that takes parameters: its name, `<`, what it holds, `>`.
                Type,
                /// One field of a `record` or a `table` type, starting with its key.
                /// Then the `:` that follows the key while nu gives it to that key.
                /// Then the one item after that `:`, which nu takes for the type whatever it is.
                TypeField,
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

    /// Whether nu reads a token of this kind as one item, such as the head of a call.
    /// An `Error` is one too: it is the item the lexer could not read.
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
    fn kind_roundtrips_through_raw() {
        for &kind in SyntaxKind::ALL {
            assert_eq!(SyntaxKind::from_raw(kind.to_raw()), kind);
        }
    }
}
