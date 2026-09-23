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

                // Operators.
                /// An operator of a `MathExpression`: `+`, `not-in`, or the `not` before a term.
                /// The parser emits this, not the lexer.
                Operator,
                /// `..`, `..=`, or `..<` of a `Range`, or the `..` before its second value.
                /// The parser emits this, not the lexer.
                RangeOperator,
                /// The `...` of a `Spread`.
                /// The parser emits this, not the lexer.
                SpreadOperator,

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
                /// An element of a `Pipeline`: a `Call`, `ExternalCall`, or `MathExpression`.
                /// Then any `Redirection`s.
                /// Where nu reads a lone value, it stands bare in place of the call.
                /// It holds an `Assignment`, a `Let`, or an `Alias` instead.
                /// A keyword nu reads before an assignment operator heads a `Call` as written.
                /// Each runs to the statement's end.
                /// Nu allows arguments after a redirection; they stay bare tokens.
                PipelineElement,
                /// A command name followed by its arguments.
                Call,
                /// A `Call` whose name starts with `^`, or is `else` or `catch`.
                /// Nu runs those two as external commands outside an `if` or a `try`.
                ExternalCall,
                /// A term, then an `Operator` and a term while the element lasts.
                /// A term is a value after the `not`s that take it, which are `Operator`s too.
                /// An item where nu takes an operator but reads none stays as it is written.
                /// An `if` or a `match` as a term is the keyword syntax over the rest of the items.
                /// The rest stays as written where that syntax does not read it.
                MathExpression,

                // Keyword syntax, read where nu reads the call as the syntax over a `Call`.
                // Each is what a `Call` would be for a command of that name.
                // A body is an argument that starts with `{`.
                /// `if`, the condition, the body, then any number of `else`, `if`, condition, body.
                /// A last `else` takes one more argument.
                /// More items after it are one expression, as what heads a statement is.
                /// The `if`s after the `else`s are in this one flat node.
                /// Each condition is any flags of its `if`, then any items left as one expression.
                /// It is a `MathExpression`, or a term of one standing alone.
                If,
                /// `for`, the variable, `in`, the value, and the body.
                /// A variable ending in `:` takes its type after it, up to the first `in`.
                /// The value is at least one argument.
                For,
                /// `while`, the condition, and the body.
                /// The condition is at least one item, read as that of an `If`.
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
                /// One arm: its patterns with `|` between them, then `if` and the guard if there.
                /// A pattern is a variable, `_`, a `ListPattern`, a `RecordPattern`, or a value.
                /// The guard is an expression, as the condition of an `If` is past its flags.
                /// A guard that holds a `|` stays as it is lexed.
                /// So does what comes before `=>` where nu rejects it.
                /// Then `=>` and the one item nu takes as what the arm does.
                MatchArm,
                /// A `Word` in `[ ]` read as a list pattern.
                /// `[`, the patterns and any `,` and comments among them, and `]`.
                /// A `..` last, alone or before a `$` and a name, is a `Word`.
                /// One nu reads less of than it holds stays a `Word`.
                ListPattern,
                /// A `Word` in `{ }` read as a record pattern.
                /// `{`, the fields and any `,` and comments among them, and `}`.
                /// A field is a `RecordField`, or a `$` and a name alone as a `Word`.
                RecordPattern,
                /// A redirection to a file: the operator, such as `o>`, and its target.
                /// Has no target only when the operator is an `Error`.
                Redirection,
                /// What a `PipelineElement` holds when an assignment operator comes before its end.
                /// The target of a file redirection is no such operator.
                /// What comes before the operator is held by a `PipelineElement` of its own.
                /// Then the operator, with any comments and line breaks before a line-start `|`.
                /// Then any `Pipeline`s of the rest, with the comments and line breaks among them.
                Assignment,
                /// As an `Assignment`, for an element that starts by declaring a variable.
                /// It starts with `let`, `mut`, `const`, or `export const`.
                /// Those words, not a `Call`, come before the arguments.
                /// A variable ending in `:` takes its type after it, up to the assignment operator.
                Let,
                /// An alias nu declares: `alias`, the name, `=`, then what the alias stands for.
                /// `export` comes before `alias` if there.
                /// Nu reads what follows `=` as one call to the statement's end.
                /// It is a `Call` or an `ExternalCall`, with any `|` and redirection as arguments.
                /// It holds the line breaks nu goes on past too.
                /// A `|` or a redirection first stays a bare token, which nu reads as the head.
                /// It is a keyword syntax, such as an `If`, where it holds no `|` or redirection.
                /// Nu declares no alias where a value heads what follows `=`.
                Alias,
                /// A `Word` in `( )` read as a subexpression: `(`, the statements, and `)`.
                /// Nu reads one wherever it takes a value, a `def` signature aside.
                Subexpression,
                /// A `Word` nu reads as a value and a path into it, as in `$x.a?` or `(ls).name`.
                /// The value, then the members and the `.`, `?`, and `!` among them, all touching.
                /// The value is what it would be alone: a node where nu reads one, or else a token.
                /// A cell path literal, as in `$.a`, has a `Word` `$` in its place.
                /// A member nu reads is a `Word` or a `String`.
                /// A token nu rejects is in an `Error`, and those past it stay as they are lexed.
                CellPath,
                /// A `Word` nu reads as a range, as in `1..10`, `$a..<$b`, or `..5`.
                /// Any first value, then any `RangeOperator` and second value.
                /// Then a `RangeOperator`, and any last value.
                /// Each value is what it would be alone, and all of them touch.
                Range,
                /// A `Word` nu reads as a spread, as the `...$x` of `[...$x]`.
                /// The `SpreadOperator`, then the value touching it.
                /// The value is a node where nu reads one, or else a token.
                /// Nu reads one among the items of a list and the fields of a record.
                /// It reads one as an argument of an external command too.
                /// One as an argument of another command stays a `Word`.
                Spread,
                /// A `Word` nu reads as a binary, as in `0x[ff 00]`.
                /// Its prefix as a `Word`, `[`, the items and any `,` and comments among them, `]`.
                /// Nu reads one where it takes any value.
                Binary,
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
                /// In a `RecordPattern`, a key, its `:`, then its pattern.
                RecordField,
                /// A `Word` in `{ }` that is no record: `{`, any parameters, the statements, `}`.
                /// Nu reads one where it takes a value or a block, or an argument of a nu command.
                /// An external command takes it as a string instead.
                /// The body of a `def` is one whatever it holds.
                Block,
                /// The parameters of a closure: `|`, what a `Signature` holds, and the next `|`.
                /// Or `||` alone.
                ClosureParams,
                /// A `def` or an `extern` that nu reads as one.
                /// `export` if there, the keyword, any flags, the name, and a `Signature`.
                /// Then the `:` and the input and output types if there.
                /// They are an `Error` where nu fails to lex them.
                /// They are an `IoTypes` where nu pairs them all up.
                /// Otherwise the items stay as they are.
                /// Then the body, which only a `def` has.
                Def,
                /// The parameters of a `def` or an `extern`.
                /// The `[` or `(`, what it holds, and the matching `]` or `)`.
                Signature,
                /// One parameter of a `Signature` or `ClosureParams`, starting with its name.
                /// Then, in source order, the `ShortFlag`, the `:` and type, the `=` and default.
                /// Each of those while nu still takes it for that parameter.
                /// It holds the `,` that ends it, and the `|`, `;`, and redirections nu ignores.
                Param,
                /// A type that takes parameters: its name, `<`, what it holds, `>`.
                /// Nu reads one in a `Signature`, in the type of a variable, and in an `IoType`.
                Type,
                /// One field of a `record` or a `table` type, starting with its key.
                /// Then the `:` that follows the key while nu gives it to that key.
                /// Then the one item after that `:`, which nu takes for the type whatever it is.
                TypeField,
                /// The input and output types of a `Def`: the `IoType`s, with any `,` among them.
                /// Written in `[ ]`, they have the `[` first and the `]` last.
                /// Comments may stand among the pairs and inside one.
                IoTypes,
                /// An input type, `->`, and an output type.
                /// Each type is a `Type` where nu reads one, or else the item as it is.
                IoType,
                /// A `module` nu reads a body in: `module`, the name as written, and the body.
                /// `export` comes before `module` if there.
                /// The body is a `Block` of statements, since nu reads no parameters there.
                /// It stays a `Word` where it holds nothing but trivia, or starts with a `|`.
                /// So it does where its statements report an error.
                Module,
                /// `export-env` and its body, the one item nu reads after it.
                /// The body is what a `{ }` is where nu takes a block.
                /// An `export-env` with no `{ }` or more than one item stays a `Call` as written.
                ExportEnv,
                /// `use`, the flags, then the import pattern: the module and its members.
                /// `export` comes before `use` if there.
                /// Nu reads the module as an argument of a command it knows.
                /// It reads each member as any value, or a `Spread` of one.
                /// A `use` with nothing after it stays a `Call`.
                Use,
                /// `where`, the flags, then the row condition: every item after them.
                /// A lone `{ }` there is a `Block`, which nu reads as a closure, or else a `Word`.
                /// Anything else is an expression, as what heads a statement is.
                /// An `if` or a `match` first takes the rest, as a term of a `MathExpression` does.
                /// A `where` with no row condition stays a `Call`.
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
    fn kind_roundtrips_through_raw() {
        for &kind in SyntaxKind::ALL {
            assert_eq!(SyntaxKind::from_raw(kind.to_raw()), kind);
        }
    }
}
