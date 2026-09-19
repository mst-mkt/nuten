//! Kinds of tokens and nodes in the CST.

/// Calls `$callback` with every kind as `tokens { .. } nodes { .. }`: the
/// one list `SyntaxKind` and `ast` are both made from, so that a node kind
/// cannot be in one and not the other. `Error` is among the tokens, and is a
/// node too.
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
                /// The parser emits this, not the lexer.
                LBracket,
                /// The parser emits this, not the lexer.
                RBracket,
                /// The parser emits this, not the lexer.
                LBrace,
                /// The parser emits this, not the lexer.
                RBrace,
                LParen,
                RParen,
                /// `<` of type parameters.
                /// The parser emits this, not the lexer.
                LAngle,
                /// `>` of type parameters.
                /// The parser emits this, not the lexer.
                RAngle,
                /// `=>` between the pattern of a `match` arm and what the arm does.
                /// The parser emits this, not the lexer.
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
                /// A `(-f)` of one character after a parameter of a signature, which nu
                /// takes for that parameter.
                /// The parser emits this, not the lexer.
                ShortFlag,
                /// An item with a lexing error, or a node wrapping a parse error.
                Error,
            }
            nodes {
                /// The whole file: its statements, with the `;` and comments among
                /// them. The only node that starts or ends with trivia.
                Root,
                /// A statement. Ends before `;`, the end of the file, or a line break,
                /// unless a `|` ends that line or, past any comment lines, starts the
                /// next.
                Pipeline,
                /// An element of a `Pipeline`: a `Call` or an `ExternalCall`, then any
                /// `Redirection`s, or else an `Assignment` or a `Let`, which runs to the
                /// end of the statement. Nu allows arguments after a redirection; they
                /// stay bare tokens. Other than an `Assignment` or a `Let`, starts with a
                /// `Redirection` instead, its operator an `Error`, when no command
                /// precedes it. Other than an `Assignment` or a `Let`, ends before a pipe
                /// (`|`, `e>|`, or `o+e>|`), `;`, a line break, or the end of the file. A
                /// pipe with nothing before or after it, as in `| ls` or `ls | | first`,
                /// yields no element.
                PipelineElement,
                /// A command name followed by its arguments.
                Call,
                /// Same as `Call`, but the name starts with `^`, or is `else` or `catch`,
                /// which nu runs as an external command outside an `if` or a `try`.
                ExternalCall,
                /// What a `Call` would be for a command named `if` that nu reads as one,
                /// with the `if`s that follow its `else`s in one flat node: `if`, the
                /// condition, the body, then any number of `else`, `if`, condition, and
                /// body, then optionally `else` and one more argument. A body is the
                /// argument right before its `else`, or the last one, and starts with
                /// `{`; a condition is at least one argument before it.
                If,
                /// What a `Call` would be for a command named `for` that nu reads as one:
                /// `for`, the variable, its type when the variable ends with `:`, `in`,
                /// at least one argument of the value, and the body, which starts with
                /// `{`.
                For,
                /// What a `Call` would be for a command named `while` that nu reads as
                /// one: `while`, at least one argument of the condition, and the body,
                /// which starts with `{`.
                While,
                /// What a `Call` would be for a command named `loop` that nu reads as
                /// one: `loop` and the body, which starts with `{`.
                Loop,
                /// What a `Call` would be for a command named `try` that nu reads as one:
                /// `try` and the body, then optionally `catch` and what it takes, each
                /// starting with `{`.
                Try,
                /// What a `Call` would be for a command named `match` that nu reads as
                /// one: `match`, at least one argument of the subject, which nu reads as
                /// any value, and the arms, which start with `{`: a `MatchArms` where nu
                /// reads arms, or else the `Word` as it is.
                Match,
                /// The `{ }` of a `match` read as arms: `{`, `MatchArm`s with the `,`
                /// between them, and `}`. Stays a `Word` when an arm has no `=>`, more
                /// or less than one item after it, or anything among its pattern but
                /// items with a `|` between two of them, such as a `;`, a `||`, or a
                /// `,`, or when a comment inside has no whitespace before its `#`.
                MatchArms,
                /// One arm: the tokens of its pattern and of any `if` guard, `|` among
                /// them, then `=>` and the one item nu takes as what the arm does.
                MatchArm,
                /// A redirection to a file: the operator, such as `o>`, and its target.
                /// Has no target only when the operator is an `Error`.
                Redirection,
                /// What a `PipelineElement` holds when an assignment operator comes
                /// before it would end, other than as the target of a file redirection:
                /// what comes before the operator as a `PipelineElement` holds it, the
                /// operator, then any comments and line breaks before a line-start `|`,
                /// and a `Pipeline` of the rest of the statement if anything follows.
                Assignment,
                /// As an `Assignment`, but for an element that starts a pipeline with
                /// `let`, `mut`, or `const`, where that word, not a `Call`, comes before
                /// the arguments. The type after a name ending in `:` is an `Error` node
                /// when nu fails to lex it.
                Let,
                /// A `Word` enclosed in `[ ]` where nu reads a list, taking any value, a
                /// block, or an argument of an external command: `[`, what it holds, and
                /// `]`. It stays a `Word` as an argument of any other command, which may
                /// read it as a string, when its `[` closes before its end, when lexing
                /// what it holds gives an `Error` or `;` token, when a comment inside
                /// has no whitespace before its `#`, or when brackets nest too deep.
                List,
                /// A `Word` enclosed in `[ ]` that nu reads as a table where it takes any
                /// value or a block: `[`, the column names, the `;` right after them, the
                /// rows, any `,` among them, and `]`. The names and each row are a
                /// `List`, or a `Word` when they hold a `;`, which nu reads as no table
                /// there. It stays a `Word` as a `List` does, as an argument of any
                /// command, and when anything else comes before the `;`, when another
                /// `;` or an item not in `[ ]` follows it, or when nothing does.
                Table,
                /// A `Word` enclosed in `{ }` that nu reads as a record where it takes
                /// any value, or where it takes a block unless a spread comes first: `{`,
                /// its fields and spreads, and `}`. It is a `Block` as the body of a
                /// `def`. It stays a `Word` as an argument of any command, which may
                /// read it as a string, when its `{` closes before its end, when lexing
                /// what it holds as a record gives an `Error`, when its fields cannot be
                /// split as nu splits them, when nu would tell it from a closure by
                /// another sign without its `,`, when a comment inside has no whitespace
                /// before its `#`, or when brackets nest too deep.
                Record,
                /// A key, then the token nu takes as its `:`, then its value, each while
                /// the record lasts. A value of several touching tokens is one `Word`.
                RecordField,
                /// A `Word` enclosed in `{ }` that is no record, read as a closure or a
                /// block where nu takes any value, a block, or an argument of a command
                /// other than an external one, and the body of a `def` whatever it
                /// holds: `{`, its parameters if any, its statements, and `}`. It stays
                /// a `Word` as an argument of an external command, when it holds nothing
                /// but trivia, when its `{` closes before its end, when its parameters
                /// meet a `;`, a shell operator, or the end before a closing `|`, when
                /// what it holds reports an error, when a comment inside has no
                /// whitespace before its `#`, or when brackets nest too deep.
                Block,
                /// The parameters of a closure as unchecked tokens: `|` to the next `|`,
                /// or `||`.
                ClosureParams,
                /// A `def` or an `extern` that nu reads as one: `export` if there, the
                /// keyword, any flags, the name, a `Signature`, the `:` and the items of
                /// the input and output types if there, and the body of a `def`.
                Def,
                /// The parameters of a `def` or an `extern`: `[`, what it holds, and `]`.
                /// Stays a `Word` when a comment inside has no whitespace before its `#`.
                Signature,
                /// One parameter of a `Signature`: its name, then in source order the
                /// `ShortFlag`, the `:` and the type, and the `=` and the default value
                /// that follow it, each while nu takes them for that parameter. It
                /// also holds the `,` that ends it, and the `|`, `;`, and redirection
                /// tokens nu ignores among them.
                Param,
                /// A type in a `Signature` that takes parameters: its name, `<`, what it
                /// holds, and `>`. Any other type stays one token, as does one with a
                /// `@`, one that holds a comment, which would swallow the `>` once the
                /// type is on one line, or one that holds a token nu refuses there.
                Type,
                /// One field of a `record` or a `table` type: its key, then the `:` that
                /// follows it while nu gives it to that key, and the one item after that
                /// `:`, which nu takes for the type whatever it is.
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
