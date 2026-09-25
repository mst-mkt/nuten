//! Kinds of tokens and nodes in the CST.

/// Calls `$callback` with every kind as `tokens { .. } nodes { .. }`.
/// `SyntaxKind` and `ast` are both made from this one list, so neither can miss a node kind.
/// `Error` is among the tokens, and is a node too.
macro_rules! syntax_kinds {
    ($callback:ident) => {
        $callback! {
            tokens {
                /// Spaces, tabs, and a `\r` not followed by `\n`.
                Whitespace,
                /// `\n` or `\r\n`.
                Newline,
                /// `#` up to the end of the line, excluding the line break.
                Comment,

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

                /// An operator of a `MathExpression`: `+`, `not-in`, or the `not` before a term.
                /// The parser emits this, not the lexer.
                Operator,
                /// `..`, `..=`, or `..<` of a `Range`, or the `..` before its second value.
                /// The parser emits this, not the lexer.
                RangeOperator,
                /// The `...` of a `Spread`.
                /// The parser emits this, not the lexer.
                SpreadOperator,

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
                /// A run of an `Interpolation` outside each `( )`, as `$"a` or `b"`.
                /// The first holds the `$` and the opening quote, and the last the closing quote.
                /// The parser emits this, not the lexer.
                InterpText,
                /// An item with a lexing error, or a node wrapping a parse error.
                Error,
            }
            nodes {
                /// The only node that starts or ends with trivia.
                Root,
                /// A statement.
                Pipeline,
                /// Where nu reads a lone value, it stands bare in place of the call.
                /// Nu allows arguments after a redirection; they stay bare tokens.
                PipelineElement,
                /// Headed by an attribute, it may go on past what ends each attribute.
                /// It holds that line break or `;` then.
                Call,
                /// A `Call` whose name starts with `^`, or is `else` or `catch`.
                /// Nu runs those two as external commands outside an `if` or a `try`.
                /// That is, unless a block defines them or an import before them may bring them.
                ExternalCall,
                /// A term, then an `Operator` and a term while the element lasts.
                /// An `if` or a `match` as a term is the keyword syntax over the rest of the items.
                MathExpression,

                // Keyword syntax, read where nu reads the call as the syntax over a `Call`.
                // Each is what a `Call` would be for a command of that name.
                // A body is an argument that starts with `{`.
                /// The `if`s after the `else`s are in this one flat node.
                If,
                For,
                While,
                Loop,
                /// `try` and the body, then optionally `catch` and the body it takes.
                Try,
                Match,
                /// The `{ }` of a `match` read as arms.
                MatchArms,
                MatchArm,
                /// A `Word` in `[ ]` read as a list pattern.
                ListPattern,
                /// A `Word` in `{ }` read as a record pattern.
                RecordPattern,
                /// A redirection to a file: the operator, such as `o>`, and its target.
                /// Has no target only when the operator is an `Error`.
                Redirection,
                /// What a `PipelineElement` holds when an assignment operator comes before its end.
                Assignment,
                /// As an `Assignment`, for an element that starts by declaring a variable.
                /// It starts with `let`, `mut`, `const`, or `export const`.
                Let,
                /// Nu reads what follows `=` as one call to the statement's end.
                /// Nu declares no alias where a value heads what follows `=`.
                Alias,
                /// A `Word` in `( )` read as a subexpression.
                Subexpression,
                /// A `Word` nu reads as a value and a path into it, as in `$x.a?` or `(ls).name`.
                /// A cell path literal, as in `$.a`, has a `Word` `$` in its place.
                CellPath,
                /// A `Word` nu reads as a range, as in `1..10`, `$a..<$b`, or `..5`.
                Range,
                /// A `Word` nu reads as a spread, as the `...$x` of `[...$x]`.
                Spread,
                /// An `InterpString` nu reads as a value, as `$"a(1 + 2)b"`.
                Interpolation,
                /// A `Word` nu reads as a binary, as in `0x[ff 00]`.
                Binary,
                /// A `Word` in `[ ]` read as a list.
                List,
                /// A `Word` in `[ ]` read as a table where nu takes a value or a block.
                /// One holding a `;` stays a `Word`, since nu reads no table there.
                Table,
                /// A `Word` in `{ }` read as a record.
                /// It is a `Block` as the body of a `def`.
                Record,
                /// A value of several touching tokens is one `Word`.
                /// In a `RecordPattern`, a key, its `:`, then its pattern.
                RecordField,
                /// A `Word` in `{ }` that is no record.
                /// The body of a `def` is one whatever it holds.
                Block,
                /// The parameters of a closure: `|`, what a `Signature` holds, and the next `|`.
                /// Or `||` alone.
                ClosureParams,
                /// An item starting with `@` where nu reads an attribute, then its arguments.
                Attribute,
                /// `Attribute`s, each with the line break or `;` that ends it after it.
                /// Where nu reads no `Def`, a `Call` of `def` or `extern` stands there instead.
                AttributeBlock,
                /// A `def` or an `extern` that nu reads as one.
                Def,
                /// The parameters of a `def` or an `extern`.
                Signature,
                /// One parameter of a `Signature` or `ClosureParams`, starting with its name.
                /// It holds the `,` that ends it, and the `|`, `;`, and redirections nu ignores.
                Param,
                /// A type that takes parameters.
                Type,
                /// One field of a `record` or a `table` type, starting with its key.
                TypeField,
                /// The input and output types of a `Def`: the `IoType`s, with any `,` among them.
                IoTypes,
                /// An input type, `->`, and an output type.
                IoType,
                /// A `module` nu reads a body in.
                /// The body is a `Block` of statements, since nu reads no parameters there.
                Module,
                /// `export-env` and its body, the one item nu reads after it.
                ExportEnv,
                /// `use`, the flags, then the import pattern: the module and its members.
                Use,
                /// `where`, the flags, then the row condition: every item after them.
                /// A lone `{ }` there is a `Block`, which nu reads as a closure, or else a `Word`.
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
    fn every_kind_roundtrips_through_raw() {
        for &kind in SyntaxKind::ALL {
            assert_eq!(SyntaxKind::from_raw(kind.to_raw()), kind);
        }
    }
}
