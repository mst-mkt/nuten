use nuten_syntax::SyntaxKind::{
    self, Assign, Colon, Comma, Comment, ErrGt, ErrGtPipe, Error, InterpString, Newline, Pipe,
    PipePipe, RawString, Semicolon, Whitespace, Word,
};
use rowan::{TextRange, TextSize};

use super::{
    LexError,
    LexErrorKind::{self, MissingRawQuote, Unclosed, UnmatchedCloser},
    Lexed, Mode, lex,
};
use crate::invariants::OPERATORS;

fn lex_all(src: &str) -> Lexed {
    lex_all_in(src, Mode::Block)
}

fn lex_all_in(src: &str, mode: Mode) -> Lexed {
    lex(src, TextRange::up_to(TextSize::of(src)), mode)
}

fn lexed(src: &str) -> Vec<(SyntaxKind, &str)> {
    lexed_in(src, Mode::Block)
}

fn lexed_in(src: &str, mode: Mode) -> Vec<(SyntaxKind, &str)> {
    lex_all_in(src, mode)
        .tokens
        .iter()
        .map(|token| (token.kind, &src[token.range]))
        .collect()
}

fn lexed_ranges(src: &str, range: TextRange) -> Vec<(SyntaxKind, TextRange)> {
    lex(src, range, Mode::Block)
        .tokens
        .iter()
        .map(|token| (token.kind, token.range))
        .collect()
}

fn error_of(src: &str) -> (LexErrorKind, &str) {
    let errors = lex_all(src).errors;
    assert_eq!(
        errors.len(),
        1,
        "{src:?} does not have one error: {errors:?}"
    );
    (errors[0].kind, &src[errors[0].range])
}

fn split(src: &str) -> Vec<&str> {
    split_in(src, Mode::Block)
}

/// What nuten lexes in `mode`, with what nu skips there dropped.
fn split_in(src: &str, mode: Mode) -> Vec<&str> {
    let skipped = skipped_in(mode);
    lexed_in(src, mode)
        .into_iter()
        .filter(|(kind, _)| !skipped.contains(kind))
        .map(|(_, text)| text)
        .collect()
}

/// What to drop from the tokens of `mode` to compare with the split nu
/// reports there.
#[expect(
    clippy::match_same_arms,
    reason = "one arm per recorded mode, to check each against nu on its own"
)]
fn skipped_in(mode: Mode) -> &'static [SyntaxKind] {
    match mode {
        Mode::Block => &[Whitespace],
        Mode::List => &[Whitespace, Newline, Comma],
        Mode::Record => &[Whitespace, Newline, Comment, Comma],
        Mode::RecordValue => &[Whitespace, Newline, Comment, Comma],
        Mode::Brace => &[Whitespace, Newline, Comment],
        Mode::VarType => &[Whitespace, Newline, Comment],
        Mode::Signature | Mode::IoTypes | Mode::TypeParams | Mode::MatchArms => {
            unimplemented!("how nu splits in {mode:?} is not recorded")
        },
    }
}

fn kind_of(src: &str) -> SyntaxKind {
    let tokens = lexed(src);
    assert_eq!(tokens.len(), 1, "{src:?} is not one token: {tokens:?}");
    tokens[0].0
}

fn assert_one_item(item: &str) {
    assert_eq!(split(&format!("{item} z")), [item, "z"], "{item:?}");
}

fn range(start: u32, end: u32) -> TextRange {
    TextRange::new(start.into(), end.into())
}

#[test]
fn empty() {
    assert!(lexed("").is_empty());
}

#[test]
fn only_trivia() {
    assert_eq!(lexed(" \t"), [(Whitespace, " \t")]);
    assert_eq!(lexed("\n"), [(Newline, "\n")]);
    assert_eq!(lexed("#"), [(Comment, "#")]);
}

#[test]
fn separators() {
    assert_eq!(
        lexed("ls | get name;a|b || c\n"),
        [
            (Word, "ls"),
            (Whitespace, " "),
            (Pipe, "|"),
            (Whitespace, " "),
            (Word, "get"),
            (Whitespace, " "),
            (Word, "name"),
            (Semicolon, ";"),
            (Word, "a"),
            (Pipe, "|"),
            (Word, "b"),
            (Whitespace, " "),
            (PipePipe, "||"),
            (Whitespace, " "),
            (Word, "c"),
            (Newline, "\n"),
        ]
    );
}

#[test]
fn pipes_pair_greedily() {
    assert_eq!(lexed("|||"), [(PipePipe, "||"), (Pipe, "|")]);
    assert_eq!(lexed("|;|"), [(Pipe, "|"), (Semicolon, ";"), (Pipe, "|")]);
}

#[test]
fn comment_tokens_start_only_at_token_start() {
    assert_eq!(lexed("foo#bar"), [(Word, "foo#bar")]);
    assert_eq!(
        lexed("foo #bar"),
        [(Word, "foo"), (Whitespace, " "), (Comment, "#bar")]
    );
    assert_eq!(lexed("a|#b"), [(Word, "a"), (Pipe, "|"), (Comment, "#b")]);
    assert_eq!(lexed("\t#c"), [(Whitespace, "\t"), (Comment, "#c")]);
}

#[test]
fn comment_runs_to_end_of_line() {
    assert_eq!(
        lexed("# a | b ; c\n#d"),
        [(Comment, "# a | b ; c"), (Newline, "\n"), (Comment, "#d")]
    );
}

#[test]
fn block_mode_has_no_special_bytes() {
    assert_eq!(lexed("a:b,c=d.e?f!"), [(Word, "a:b,c=d.e?f!")]);
    assert_eq!(lexed("--foo=bar"), [(Word, "--foo=bar")]);
    assert_eq!(lexed(",a"), [(Word, ",a")]);
}

#[test]
fn non_ascii_bytes_belong_to_words() {
    assert_eq!(
        lexed("日本 語\t✓"),
        [
            (Word, "日本"),
            (Whitespace, " "),
            (Word, "語"),
            (Whitespace, "\t"),
            (Word, "✓"),
        ]
    );
    assert_eq!(lexed("#日本\n"), [(Comment, "#日本"), (Newline, "\n")]);
    assert_eq!(lexed("|日;"), [(Pipe, "|"), (Word, "日"), (Semicolon, ";")]);
    assert_eq!(lexed("a\u{85}b\u{a0}c"), [(Word, "a\u{85}b\u{a0}c")]);
}

#[test]
fn terminators_do_not_end_nested_items() {
    for item in ["[a\n| b; c]", "{a (b | c)}", "'a | b;c'"] {
        assert_one_item(item);
    }
}

#[test]
fn strings_inside_brackets_hide_closers() {
    for item in [r#"[a "]" b]"#, "[r#'a']'#]"] {
        assert_one_item(item);
    }
}

#[test]
fn backslash_escapes_only_in_double_quotes() {
    assert_eq!(kind_of(r#""a\"b""#), SyntaxKind::String);
    assert_eq!(kind_of(r"'a\'"), SyntaxKind::String);
    assert_eq!(kind_of(r"`a\`"), SyntaxKind::String);
}

#[test]
fn interpolation_nests_quotes_and_parens() {
    for item in [
        r#"$"a(")")b""#,
        r#"$"((1) + ")")""#,
        r#"$"('"')""#,
        r#"$"(`"`)""#,
        r#"$"('(')""#,
        r#"$"("\"")""#,
        r#"$"(\)""#,
        "$'a(')')b'",
        r#"x$"(")")""#,
    ] {
        assert_one_item(item);
    }
}

#[test]
fn other_parens_in_strings_are_plain() {
    for item in [r#""a(""#, r#"$"a\(""#, "$`a(`"] {
        assert_one_item(item);
    }
}

#[test]
fn comment_inside_brackets_hides_delimiters() {
    for item in ["[a # ] b\n]", "[a # ] b\r]", "[a # b#] c\n]"] {
        assert_one_item(item);
    }
    assert_eq!(split("[a#] b]"), ["[a#]", "b]"]);
}

/// nu reads the byte before `#` as Latin-1.
/// `à` (`C3 A0`) ends in whitespace, so a comment starts inside the item.
#[test]
fn hash_after_latin1_whitespace_byte_starts_comment() {
    assert_eq!(split(r#"à#"b c"#), [r#"à#"b"#, "c"]);
    assert_eq!(split(r#"a#"b c""#), [r#"a#"b c""#]);
}

#[test]
fn raw_string_closes_at_the_first_closer() {
    assert_eq!(kind_of("r##'a '# b'##"), RawString);
    assert_eq!(split("r#'a'# r#'b'#"), ["r#'a'#", "r#'b'#"]);
    assert_eq!(kind_of("r#'#"), RawString);
}

#[test]
fn pipe_joins_a_redirection() {
    assert_eq!(split("e>|cat"), ["e>|", "cat"]);
    assert_eq!(split("o+e>|cat"), ["o+e>|", "cat"]);
    assert_eq!(split("e>>|cat"), ["e>>", "|", "cat"]);
    assert_eq!(split("xe>|cat"), ["xe>", "|", "cat"]);
}

#[test]
fn single_literal_items_get_literal_kinds() {
    assert_eq!(kind_of("'a b'"), SyntaxKind::String);
    assert_eq!(kind_of("`a`"), SyntaxKind::String);
    assert_eq!(kind_of(r#"$"a(1 + ")")""#), InterpString);
    assert_eq!(kind_of("$'a(1)'"), InterpString);
    assert_eq!(kind_of("r#'a'#"), RawString);
}

#[test]
fn other_items_are_words() {
    for item in [
        r#"a"b c"d"#,
        r#""a"b"#,
        r#""a"'b'"#,
        r#"$"a"b"#,
        r#""a"r#'b'#"#,
        r#"r#'a'#"b""#,
        "r#'a'#b",
        "$`a`",
        r#"r"a""#,
        "[a b]",
        "(1 + 2)",
    ] {
        assert_eq!(kind_of(item), Word, "{item:?}");
    }
}

#[test]
fn operators_get_their_kinds() {
    for &(expected, spellings) in OPERATORS {
        for &text in spellings {
            assert_eq!(kind_of(text), expected, "{text:?}");
        }
    }
    // nu reports these, and nuten leaves them to the parser.
    for text in ["o>|", "out>|", "&&", "2>", "2>&1"] {
        assert_eq!(kind_of(text), Word, "{text:?}");
    }
}

#[test]
fn unclosed_item_runs_to_the_end() {
    assert_eq!(
        lexed(r#"a "b c"#),
        [(Word, "a"), (Whitespace, " "), (Error, r#""b c"#)]
    );
    assert_eq!(kind_of("[a\n(b c"), Error);
    assert_eq!(kind_of(r#"$"(a" b"#), Error);
    assert_eq!(kind_of("r#'a' b"), Error);
}

#[test]
fn error_ends_the_item_where_found() {
    assert_eq!(lexed("a}b"), [(Error, "a}"), (Word, "b")]);
    assert_eq!(lexed("a)b"), [(Error, "a)"), (Word, "b")]);
    assert_eq!(lexed("[a)b]"), [(Error, "[a)"), (Word, "b]")]);
    assert_eq!(lexed("{[}]"), [(Error, "{[}"), (Word, "]")]);
    assert_eq!(lexed("r#x"), [(Error, "r#"), (Word, "x")]);
}

#[test]
fn final_backslash_splits_off_only_from_the_outermost_string() {
    assert_eq!(lexed(r#""a\"#), [(Error, r#""a"#), (Word, r"\")]);
    assert_eq!(lexed(r#"$"("\"#), [(Error, r#"$"("\"#)]);
}

#[test]
fn unclosed_points_at_the_innermost_opener() {
    for (src, opener) in [
        (r#""a b"#, "\""),
        ("'a", "'"),
        ("$`a", "`"),
        ("a[b", "["),
        (r#"{a "b""#, "{"),
        ("[a\n(b c", "("),
        (r#"[a "b"#, "\""),
        (r#""a\"#, "\""),
        (r#"$"(a)b"#, r#"$""#),
    ] {
        assert_eq!(error_of(src), (Unclosed, opener), "{src:?}");
    }
}

#[test]
fn unclosed_interpolation_points_at_the_outermost_paren() {
    for (src, paren) in [
        ("$'(a", 2),
        (r#"$"(a" b"#, 2),
        (r#"$"a(b(c"#, 3),
        (r#"$"(a)(b"#, 5),
    ] {
        assert_eq!(
            lex_all(src).errors,
            [LexError {
                kind: Unclosed,
                range: range(paren, paren + 1),
            }],
            "{src:?}"
        );
    }
}

#[test]
fn unmatched_closer_points_at_the_closer() {
    for (src, closer) in [
        ("a}b", "}"),
        ("a)b", ")"),
        ("[a)b]", ")"),
        ("{[}]", "}"),
        ("(a]", "]"),
    ] {
        assert_eq!(error_of(src), (UnmatchedCloser, closer), "{src:?}");
    }
}

/// How nu 0.115.1 splits each input.
const NU_SPLITS: &[(&str, &[&str])] = &[
    ("foo#bar", &["foo#bar"]),
    ("foo #bar", &["foo", "#bar"]),
    ("echo [a b]#c", &["echo", "[a b]#c"]),
    (r##"echo "a"#b"##, &["echo", r##""a"#b"##]),
    ("echo a:b", &["echo", "a:b"]),
    ("echo a,b", &["echo", "a,b"]),
    ("echo --foo=bar", &["echo", "--foo=bar"]),
    (r#"echo a"b"c"#, &["echo", r#"a"b"c"#]),
    (r#"echo ab"c d"e"#, &["echo", r#"ab"c d"e"#]),
    (r#"echo "a"b"#, &["echo", r#""a"b"#]),
    (r"echo 'a\'b", &["echo", r"'a\'b"]),
    (r#"echo "a\"#, &["echo", r#""a"#, r"\"]),
    (r"echo a\ b", &["echo", r"a\", "b"]),
    (r#"echo $"x(1 + 2)y""#, &["echo", r#"$"x(1 + 2)y""#]),
    (r#"echo $"x\(1)y""#, &["echo", r#"$"x\(1)y""#]),
    ("echo $'x(1)y'", &["echo", "$'x(1)y'"]),
    ("echo $`x(1)y`", &["echo", "$`x(1)y`"]),
    ("echo r#'a'b'#", &["echo", "r#'a'b'#"]),
    ("echo r##'a'#b'##", &["echo", "r##'a'#b'##"]),
    ("echo r#'a\nb'#", &["echo", "r#'a\nb'#"]),
    ("echo r#x", &["echo", "r#", "x"]),
    ("echo r#'#", &["echo", "r#'#"]),
    ("echo xr#'a'#", &["echo", "xr#'a'#"]),
    ("echo a|b", &["echo", "a", "|", "b"]),
    ("echo a;b", &["echo", "a", ";", "b"]),
    ("echo x || y", &["echo", "x", "||", "y"]),
    ("echo a[b]", &["echo", "a[b]"]),
    ("echo a]b", &["echo", "a]b"]),
    ("echo a}b", &["echo", "a}", "b"]),
    ("echo [a]b", &["echo", "[a]b"]),
    ("echo (1)x", &["echo", "(1)x"]),
    ("echo {a:1}b", &["echo", "{a:1}b"]),
    ("echo o> f", &["echo", "o>", "f"]),
    ("echo e>| cat", &["echo", "e>|", "cat"]),
    ("echo a\tb", &["echo", "a", "b"]),
    ("$x = 1", &["$x", "=", "1"]),
    ("echo a += 1", &["echo", "a", "+=", "1"]),
    (
        "@attr\ndef f [] {}",
        &["@attr", "\n", "def", "f", "[]", "{}"],
    ),
];

/// The rows where nu reports a lexing error.
const NU_ERRORS: &[(&str, LexErrorKind)] = &[
    (r#"echo "a\"#, Unclosed),
    ("echo r#x", MissingRawQuote),
    ("echo a}b", UnmatchedCloser),
];

#[test]
fn splits_match_nu() {
    for &(src, expected) in NU_SPLITS {
        assert_eq!(split(src), expected, "{src:?}");
    }
}

/// How nu 0.115.1 splits the inside of a list.
const NU_LIST_SPLITS: &[(&str, &[&str])] = &[
    ("a,b", &["a", "b"]),
    ("a,,b", &["a", "b"]),
    ("a , b", &["a", "b"]),
    ("a,", &["a"]),
    (",a", &["a"]),
    (" , ", &[]),
    ("a,\nb", &["a", "b"]),
    ("a\r,b", &["a", "b"]),
    ("a,|b", &["a", "|", "b"]),
    ("a=b", &["a=b"]),
    ("a:b", &["a:b"]),
    ("--x=1", &["--x=1"]),
    (r#""a,b""#, &[r#""a,b""#]),
    ("a[b,c]d", &["a[b,c]d"]),
    ("a[b,#c] d", &["a[b,#c]", "d"]),
    ("[a #,] b\n],c", &["[a #,] b\n]", "c"]),
    ("a#b,c", &["a#b", "c"]),
    ("à#b,c", &["à#b", "c"]),
    ("a},b", &["a}", "b"]),
    ("[a,b", &["[a,b"]),
];

#[test]
fn list_splits_match_nu() {
    for &(src, expected) in NU_LIST_SPLITS {
        assert_eq!(split_in(src, Mode::List), expected, "{src:?}");
    }
}

/// How nu 0.115.1 splits the inside of a record where it reads a key or
/// its `:`.
const NU_RECORD_SPLITS: &[(&str, &[&str])] = &[
    ("a: 1", &["a", ":", "1"]),
    ("a:1,b:2", &["a", ":", "1", "b", ":", "2"]),
    (r#""a:b": 1"#, &[r#""a:b""#, ":", "1"]),
    (r#"a: "x:y""#, &["a", ":", r#""x:y""#]),
    (":a", &[":", "a"]),
    ("a#b: 1", &["a#b", ":", "1"]),
    ("à#b:c", &["à#b", ":", "c"]),
    ("a: [1, 2]", &["a", ":", "[1, 2]"]),
    ("a: {b: 1}", &["a", ":", "{b: 1}"]),
    ("a: 1\nb: 2", &["a", ":", "1", "b", ":", "2"]),
    ("a: 1 # c: d\nb: 2", &["a", ":", "1", "b", ":", "2"]),
    ("a=b.c?d!", &["a=b.c?d!"]),
];

#[test]
fn record_splits_match_nu() {
    for &(src, expected) in NU_RECORD_SPLITS {
        assert_eq!(split_in(src, Mode::Record), expected, "{src:?}");
    }
}

/// How nu 0.115.1 splits the inside of `{ }` to tell a record from a
/// closure or a block.
const NU_BRACE_SPLITS: &[(&str, &[&str])] = &[
    ("a: 1", &["a", ":", "1"]),
    ("a :b", &["a", ":", "b"]),
    ("a,b: 1", &["a,b", ":", "1"]),
    ("a\n: 1", &["a", ":", "1"]),
    ("a # b: c\n: 1", &["a", ":", "1"]),
    ("|x| a", &["|", "x", "|", "a"]),
    ("...$x, a: 1", &["...$x,", "a", ":", "1"]),
    ("a=b.c?d!", &["a=b.c?d!"]),
];

#[test]
fn brace_splits_match_nu() {
    for &(src, expected) in NU_BRACE_SPLITS {
        assert_eq!(split_in(src, Mode::Brace), expected, "{src:?}");
    }
}

/// How nu 0.115.1 splits a value of a record.
const NU_RECORD_VALUE_SPLITS: &[(&str, &[&str])] = &[
    ("b:c", &["b:c"]),
    ("b:c,d", &["b:c", "d"]),
    ("x:#c", &["x:#c"]),
    ("b:c #x\nd", &["b:c", "d"]),
    ("b\r\nc", &["b", "c"]),
];

#[test]
fn record_value_splits_match_nu() {
    for &(src, expected) in NU_RECORD_VALUE_SPLITS {
        assert_eq!(split_in(src, Mode::RecordValue), expected, "{src:?}");
    }
}

/// How nu 0.115.1 splits the type of a variable.
const NU_VAR_TYPE_SPLITS: &[(&str, &[&str])] = &[
    ("record<a: int, b: string>", &["record<a: int, b: string>"]),
    ("int, string", &["int", ",", "string"]),
    ("a,<b,c>", &["a", ",", "<b,c>"]),
    ("a< b >c d", &["a< b >c", "d"]),
    ("a<(b)>", &["a<(b)>"]),
    ("a>b", &["a>b"]),
    ("a:b", &["a:b"]),
];

#[test]
fn var_type_splits_match_nu() {
    for &(src, expected) in NU_VAR_TYPE_SPLITS {
        assert_eq!(split_in(src, Mode::VarType), expected, "{src:?}");
    }
}

#[test]
fn angle_brackets_nest_in_a_var_type() {
    for (src, error) in [
        ("a<b", (Unclosed, "<")),
        ("a<b<c>", (Unclosed, "<")),
        ("a<[b>]", (Unclosed, "<")),
        ("a<b # c>", (Unclosed, "<")),
        ("a<b]", (UnmatchedCloser, "]")),
        ("a<b}", (UnmatchedCloser, "}")),
        ("a<b)", (UnmatchedCloser, ")")),
    ] {
        let Lexed {
            tokens,
            errors,
        } = lex_all_in(src, Mode::VarType);
        assert_eq!(tokens.len(), 1, "{src:?}");
        assert_eq!(errors.len(), 1, "{src:?}");
        assert_eq!((errors[0].kind, &src[errors[0].range]), error, "{src:?}");
    }
    for mode in [
        Mode::Block,
        Mode::List,
        Mode::Record,
        Mode::RecordValue,
        Mode::Brace,
        Mode::MatchArms,
    ] {
        assert_eq!(lexed_in("a<b", mode), [(Word, "a<b")], "{mode:?}");
    }
}

#[test]
fn a_colon_is_a_token_in_record_and_brace_modes() {
    for mode in [Mode::Record, Mode::Brace] {
        assert_eq!(
            lexed_in("a:b", mode),
            [(Word, "a"), (Colon, ":"), (Word, "b")],
            "{mode:?}"
        );
    }
}

#[test]
fn special_bytes_are_tokens_at_the_start_of_an_item() {
    assert_eq!(
        lexed_in(",a,,b ,", Mode::List),
        [
            (Comma, ","),
            (Word, "a"),
            (Comma, ","),
            (Comma, ","),
            (Word, "b"),
            (Whitespace, " "),
            (Comma, ","),
        ]
    );
}

#[test]
fn items_ending_at_a_special_byte_keep_their_kinds() {
    for (src, kind) in [
        (r#""a",b"#, SyntaxKind::String),
        (r#"$"(a,b)",b"#, InterpString),
        ("r#'a,b'#,b", RawString),
        ("e>,b", ErrGt),
        ("e>|,b", ErrGtPipe),
        ("=,b", Assign),
    ] {
        let tokens = lexed_in(src, Mode::List);
        assert_eq!(tokens[0].0, kind, "{src:?}");
        assert_eq!(&tokens[1..], [(Comma, ","), (Word, "b")], "{src:?}");
    }
}

#[test]
fn errors_match_nu() {
    for &(src, _) in NU_SPLITS {
        let kinds: Vec<_> = lex_all(src).errors.iter().map(|error| error.kind).collect();
        let expected = NU_ERRORS
            .iter()
            .find(|&&(row, _)| row == src)
            .map(|&(_, kind)| kind);
        assert_eq!(kinds, expected.as_slice(), "{src:?}");
    }
}

#[test]
fn range_is_absolute() {
    assert_eq!(
        lexed_ranges("ab cd ef", range(3, 6)),
        [(Word, range(3, 5)), (Whitespace, range(5, 6))]
    );
}

#[test]
fn range_cuts_through_tokens() {
    assert_eq!(lexed_ranges("abcd", range(1, 3)), [(Word, range(1, 3))]);
    assert_eq!(
        lexed_ranges("a\r\n", range(0, 2)),
        [(Word, range(0, 1)), (Whitespace, range(1, 2))]
    );
    assert_eq!(
        lexed_ranges(r#""a b""#, range(0, 3)),
        [(Error, range(0, 3))]
    );
    assert!(lexed_ranges("abcd", range(2, 2)).is_empty());
}

#[test]
fn bytes_before_the_range_are_ignored() {
    assert_eq!(
        lexed_ranges(r#"$"(" x"#, range(1, 6)),
        [
            (SyntaxKind::String, range(1, 4)),
            (Whitespace, range(4, 5)),
            (Word, range(5, 6)),
        ]
    );
    assert_eq!(
        lex(r#"$"a"#, range(1, 3), Mode::Block).errors,
        [LexError {
            kind: Unclosed,
            range: range(1, 2),
        }]
    );
}

#[test]
#[should_panic(expected = "range splits a char")]
fn range_must_be_on_char_boundaries() {
    lex("日", range(0, 1), Mode::Block);
}

#[test]
#[should_panic(expected = "range is outside src")]
fn range_must_be_within_src() {
    lex("ab", range(1, 3), Mode::Block);
}

mod nu_differences {
    use super::*;

    #[test]
    fn crlf_is_one_newline() {
        assert_eq!(lexed("\r\n"), [(Newline, "\r\n")]);
        assert_eq!(
            lexed("a\r\nb"),
            [(Word, "a"), (Newline, "\r\n"), (Word, "b")]
        );
        assert_eq!(
            lexed("a \r\nb"),
            [
                (Word, "a"),
                (Whitespace, " "),
                (Newline, "\r\n"),
                (Word, "b")
            ]
        );
        assert_eq!(lexed("\r\r\n"), [(Whitespace, "\r"), (Newline, "\r\n")]);
    }

    #[test]
    fn lone_cr_is_whitespace() {
        assert_eq!(lexed("\r"), [(Whitespace, "\r")]);
        assert_eq!(
            lexed("a \r\tb"),
            [(Word, "a"), (Whitespace, " \r\t"), (Word, "b")]
        );
        assert_eq!(
            lexed("a\rb"),
            [(Word, "a"), (Whitespace, "\r"), (Word, "b")]
        );
    }

    #[test]
    fn comment_keeps_cr_except_before_lf() {
        assert_eq!(lexed("# c\rd"), [(Comment, "# c\rd")]);
        assert_eq!(lexed("# c\r"), [(Comment, "# c\r")]);
        assert_eq!(
            lexed("# c\r\nd"),
            [(Comment, "# c"), (Newline, "\r\n"), (Word, "d")]
        );
        assert_eq!(lexed("#\r\r\n"), [(Comment, "#\r"), (Newline, "\r\n")]);
    }

    #[test]
    fn leading_pipe_is_not_rewritten() {
        assert_eq!(
            lexed("a\n| b"),
            [
                (Word, "a"),
                (Newline, "\n"),
                (Pipe, "|"),
                (Whitespace, " "),
                (Word, "b"),
            ]
        );
    }

    #[test]
    fn unclosed_opener_includes_its_prefix() {
        for (src, opener) in [
            (r#"$"a"#, r#"$""#),
            ("x$'a", "$'"),
            ("r#'a' b", "r#'"),
            ("r##'a'#", "r##'"),
        ] {
            assert_eq!(error_of(src), (Unclosed, opener), "{src:?}");
        }
    }

    #[test]
    fn missing_raw_quote_points_at_the_prefix() {
        assert_eq!(error_of("r#x"), (MissingRawQuote, "r#"));
        assert_eq!(error_of("xr##"), (MissingRawQuote, "r##"));
    }
}
