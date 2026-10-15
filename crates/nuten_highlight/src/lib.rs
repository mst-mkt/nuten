mod literal;

use std::iter::successors;

use nuten_parser::is_parser_keyword;
use nuten_syntax::{
    NodeOrToken, SyntaxElement, SyntaxKind, SyntaxNode, SyntaxToken, TextRange, TextSize,
};

use crate::literal::is_number_like;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TokenType {
    Comment,
    Keyword,
    Function,
    Variable,
    Parameter,
    Flag,
    String,
    Number,
    Operator,
    Type,
    Property,
}

impl TokenType {
    pub const ALL: [TokenType; 11] = [
        TokenType::Comment,
        TokenType::Keyword,
        TokenType::Function,
        TokenType::Variable,
        TokenType::Parameter,
        TokenType::Flag,
        TokenType::String,
        TokenType::Number,
        TokenType::Operator,
        TokenType::Type,
        TokenType::Property,
    ];

    pub fn name(self) -> &'static str {
        match self {
            TokenType::Comment => "comment",
            TokenType::Keyword => "keyword",
            TokenType::Function => "function",
            TokenType::Variable => "variable",
            TokenType::Parameter => "parameter",
            TokenType::Flag => "flag",
            TokenType::String => "string",
            TokenType::Number => "number",
            TokenType::Operator => "operator",
            TokenType::Type => "type",
            TokenType::Property => "property",
        }
    }
}

pub fn highlight(root: &SyntaxNode) -> Vec<(TextRange, TokenType)> {
    root.descendants_with_tokens()
        .filter_map(NodeOrToken::into_token)
        .filter_map(|token| {
            let token_type = token_type(&token)?;
            Some((range_of(&token, token_type), token_type))
        })
        .collect()
}

fn range_of(token: &SyntaxToken, token_type: TokenType) -> TextRange {
    let range = token.text_range();
    if token_type == TokenType::Variable && token.text().len() > 1 && token.text().ends_with(':') {
        TextRange::new(range.start(), range.end() - TextSize::from(1))
    } else {
        range
    }
}

fn token_type(token: &SyntaxToken) -> Option<TokenType> {
    match token.kind() {
        SyntaxKind::Comment => Some(TokenType::Comment),
        SyntaxKind::String
        | SyntaxKind::RawString
        | SyntaxKind::InterpString
        | SyntaxKind::InterpText => Some(string_type(token)),
        SyntaxKind::Error if opens_string(token.text()) => Some(TokenType::String),
        SyntaxKind::ShortFlag => Some(TokenType::Flag),
        SyntaxKind::Operator
        | SyntaxKind::RangeOperator
        | SyntaxKind::SpreadOperator
        | SyntaxKind::Assign
        | SyntaxKind::Eq
        | SyntaxKind::FatArrow
        | SyntaxKind::OutGt
        | SyntaxKind::OutGtGt
        | SyntaxKind::ErrGt
        | SyntaxKind::ErrGtGt
        | SyntaxKind::OutErrGt
        | SyntaxKind::OutErrGtGt
        | SyntaxKind::ErrGtPipe
        | SyntaxKind::OutErrGtPipe => Some(TokenType::Operator),
        SyntaxKind::Pipe if parent_kind(token) == SyntaxKind::Pipeline => Some(TokenType::Operator),
        SyntaxKind::Word => word_type(token),
        _ => None,
    }
}

fn string_type(token: &SyntaxToken) -> TokenType {
    if matches!(parent_kind(token), SyntaxKind::Def | SyntaxKind::Alias) {
        TokenType::Function
    } else {
        TokenType::String
    }
}

fn opens_string(text: &str) -> bool {
    text.starts_with(['"', '\'', '`'])
        || text.starts_with("$\"")
        || text.starts_with("$'")
        || text.starts_with("r#")
}

fn word_type(token: &SyntaxToken) -> Option<TokenType> {
    let parent = token.parent()?;
    let text = token.text();
    let is_keyword = || keywords_of(parent.kind()).contains(&text);
    let first = is_first(token);
    let after_colon = right_after(token, SyntaxKind::Colon);
    match parent.kind() {
        SyntaxKind::Call if first => Some(head_type(text)),
        SyntaxKind::ExternalCall | SyntaxKind::Attribute if first => Some(TokenType::Function),
        SyntaxKind::CommandName => Some(command_name_type(&parent)),
        SyntaxKind::Binary => Some(TokenType::Number),
        _ if is_keyword() => Some(TokenType::Keyword),
        SyntaxKind::Def | SyntaxKind::Alias if !text.starts_with('-') => Some(TokenType::Function),
        SyntaxKind::Def => Some(TokenType::Flag),
        SyntaxKind::Let => Some(name_or_type(token, &parent)),
        SyntaxKind::For if !after_word(token, "in") => Some(name_or_type(token, &parent)),
        SyntaxKind::IoType if text == "->" => Some(TokenType::Operator),
        SyntaxKind::Type | SyntaxKind::IoType => Some(TokenType::Type),
        SyntaxKind::Param | SyntaxKind::TypeField if after_colon => Some(TokenType::Type),
        SyntaxKind::Param if first && text.starts_with('-') => Some(TokenType::Flag),
        SyntaxKind::Param if first => Some(TokenType::Parameter),
        SyntaxKind::TypeField => Some(TokenType::Property),
        SyntaxKind::RecordField if first && !text.starts_with(['$', '(']) => {
            Some(TokenType::Property)
        },
        SyntaxKind::CellPath if right_after(token, SyntaxKind::Dot) => Some(TokenType::Property),
        SyntaxKind::Call | SyntaxKind::ExternalCall => literal_type(text, true),
        _ => literal_type(text, false),
    }
}

fn literal_type(text: &str, in_call: bool) -> Option<TokenType> {
    if text.starts_with('$') || text.starts_with("..$") {
        Some(TokenType::Variable)
    } else if is_number_like(text) {
        Some(TokenType::Number)
    } else if matches!(text, "true" | "false" | "null") {
        Some(TokenType::Keyword)
    } else if in_call && text.len() > 1 && text.starts_with('-') {
        Some(TokenType::Flag)
    } else {
        None
    }
}

fn head_type(text: &str) -> TokenType {
    if is_number_like(text) {
        TokenType::Number
    } else if is_parser_keyword(text) {
        TokenType::Keyword
    } else {
        TokenType::Function
    }
}

fn command_name_type(name: &SyntaxNode) -> TokenType {
    let words: Vec<_> = name
        .children_with_tokens()
        .filter_map(NodeOrToken::into_token)
        .filter(|token| token.kind() == SyntaxKind::Word)
        .map(|token| token.text().to_owned())
        .collect();
    if is_parser_keyword(&words.join(" ")) {
        TokenType::Keyword
    } else {
        TokenType::Function
    }
}

fn keywords_of(kind: SyntaxKind) -> &'static [&'static str] {
    match kind {
        SyntaxKind::If => &["if", "else"],
        SyntaxKind::For => &["for", "in"],
        SyntaxKind::While => &["while"],
        SyntaxKind::Loop => &["loop"],
        SyntaxKind::Try => &["try", "catch"],
        SyntaxKind::Match => &["match"],
        SyntaxKind::MatchArm => &["if"],
        SyntaxKind::Let => &["export", "let", "mut", "const"],
        SyntaxKind::Alias => &["export", "alias"],
        SyntaxKind::Def => &["export", "def", "extern"],
        SyntaxKind::Module => &["export", "module"],
        SyntaxKind::ExportEnv => &["export-env"],
        SyntaxKind::Use => &["export", "use"],
        SyntaxKind::Where => &["where"],
        _ => &[],
    }
}

fn name_or_type(token: &SyntaxToken, parent: &SyntaxNode) -> TokenType {
    let keywords = keywords_of(parent.kind());
    let name = parent
        .children_with_tokens()
        .filter_map(NodeOrToken::into_token)
        .find(|word| word.kind() == SyntaxKind::Word && !keywords.contains(&word.text()));
    if name.as_ref() == Some(token) {
        TokenType::Variable
    } else {
        TokenType::Type
    }
}

fn parent_kind(token: &SyntaxToken) -> SyntaxKind {
    token
        .parent()
        .map_or(SyntaxKind::Root, |parent| parent.kind())
}

fn preceding(token: &SyntaxToken) -> impl Iterator<Item = SyntaxElement> {
    successors(
        token.prev_sibling_or_token(),
        SyntaxElement::prev_sibling_or_token,
    )
}

fn is_first(token: &SyntaxToken) -> bool {
    preceding(token).all(|sibling| sibling.kind().is_trivia())
}

fn right_after(token: &SyntaxToken, kind: SyntaxKind) -> bool {
    preceding(token)
        .find(|sibling| !sibling.kind().is_trivia())
        .is_some_and(|sibling| sibling.kind() == kind)
}

fn after_word(token: &SyntaxToken, text: &str) -> bool {
    preceding(token)
        .filter_map(NodeOrToken::into_token)
        .any(|sibling| sibling.kind() == SyntaxKind::Word && sibling.text() == text)
}

#[cfg(test)]
mod tests {
    use super::{TokenType, highlight};

    fn typed(src: &str) -> Vec<(&str, &'static str)> {
        let root = nuten_parser::parse(src).syntax();
        highlight(&root)
            .into_iter()
            .map(|(range, token_type)| (&src[range], token_type.name()))
            .collect()
    }

    #[test]
    fn token_types_are_named_as_in_lsp_but_flag() {
        let names: Vec<_> = TokenType::ALL
            .iter()
            .map(|token_type| token_type.name())
            .collect();
        assert_eq!(
            names,
            [
                "comment",
                "keyword",
                "function",
                "variable",
                "parameter",
                "flag",
                "string",
                "number",
                "operator",
                "type",
                "property",
            ]
        );
    }

    #[test]
    fn comments_and_strings_of_every_quote_are_typed() {
        assert_eq!(
            typed("# c\necho 'a' \"b\" `c` r#'d'# $\"e(1)\""),
            [
                ("# c", "comment"),
                ("echo", "function"),
                ("'a'", "string"),
                ("\"b\"", "string"),
                ("`c`", "string"),
                ("r#'d'#", "string"),
                ("$\"e", "string"),
                ("1", "number"),
                ("\"", "string"),
            ]
        );
    }

    #[test]
    fn unclosed_string_is_string_to_end_of_source() {
        for quote in ["\"", "'", "`", "$\"", "$'", "r#'"] {
            let src = format!("print {quote}hello\nls");
            let string = &src["print ".len()..];
            assert_eq!(
                typed(&src),
                [("print", "function"), (string, "string")],
                "{quote}"
            );
        }
    }

    #[test]
    fn unclosed_bracket_is_no_string() {
        assert_eq!(typed("echo [1"), [("echo", "function")]);
    }

    #[test]
    fn keywords_of_each_syntax_are_keywords() {
        let keywords = |src| -> Vec<&str> {
            typed(src)
                .into_iter()
                .filter(|&(_, name)| name == "keyword")
                .map(|(text, _)| text)
                .collect()
        };
        assert_eq!(
            keywords("if $a { } else if $b { } else { }"),
            ["if", "else", "if", "else"]
        );
        assert_eq!(keywords("for x in $xs { }"), ["for", "in"]);
        assert_eq!(keywords("while $a { }; loop { }"), ["while", "loop"]);
        assert_eq!(keywords("try { } catch { }"), ["try", "catch"]);
        assert_eq!(keywords("match $a { $b if $c => 1 }"), ["match", "if"]);
        assert_eq!(
            keywords("let a = 1; mut b = 1; export const c = 1"),
            ["let", "mut", "export", "const"]
        );
        assert_eq!(
            keywords("export def f [] { }; extern g []"),
            ["export", "def", "extern"]
        );
        assert_eq!(keywords("export alias a = ls"), ["export", "alias"]);
        assert_eq!(
            keywords("export module m { export use std }"),
            ["export", "module", "export", "use"]
        );
        assert_eq!(keywords("export-env { }"), ["export-env"]);
        assert_eq!(keywords("ls | where $it > 1"), ["where"]);
    }

    #[test]
    fn syntax_with_parts_left_to_write_keeps_its_keyword() {
        assert_eq!(typed("if $a"), [("if", "keyword"), ("$a", "variable")]);
        assert_eq!(typed("let"), [("let", "keyword")]);
        assert_eq!(
            typed("export def"),
            [("export", "keyword"), ("def", "keyword")]
        );
    }

    #[test]
    fn head_of_call_is_function_unless_nu_reads_keyword_or_value() {
        assert_eq!(typed("ls"), [("ls", "function")]);
        assert_eq!(typed("^git"), [("^git", "function")]);
        assert_eq!(typed("return 1"), [("return", "keyword"), ("1", "number")]);
        assert_eq!(
            typed("overlay use m"),
            [("overlay", "keyword"), ("use", "keyword")]
        );
        assert_eq!(
            typed("overlay list"),
            [("overlay", "function"), ("list", "function")]
        );
        assert_eq!(
            typed("let d = 5sec"),
            [
                ("let", "keyword"),
                ("d", "variable"),
                ("=", "operator"),
                ("5sec", "number"),
            ]
        );
    }

    #[test]
    fn else_outside_if_is_external_command() {
        assert_eq!(typed("else"), [("else", "function")]);
    }

    #[test]
    fn names_of_definitions_and_attributes_are_functions() {
        assert_eq!(
            typed("def --env \"str shout\" [] { }"),
            [
                ("def", "keyword"),
                ("--env", "flag"),
                ("\"str shout\"", "function"),
            ]
        );
        assert_eq!(
            typed("alias \"l l\" = ls"),
            [
                ("alias", "keyword"),
                ("\"l l\"", "function"),
                ("=", "operator"),
                ("ls", "function")
            ]
        );
        assert_eq!(
            typed("alias ll = ls -l"),
            [
                ("alias", "keyword"),
                ("ll", "function"),
                ("=", "operator"),
                ("ls", "function"),
                ("-l", "flag"),
            ]
        );
        assert_eq!(
            typed("@example \"x\" { f }\ndef f [] { }")[..3],
            [
                ("@example", "function"),
                ("\"x\"", "string"),
                ("f", "function"),
            ]
        );
    }

    #[test]
    fn names_that_let_and_for_bind_are_variables_before_their_types() {
        assert_eq!(
            typed("let x: int = 1"),
            [
                ("let", "keyword"),
                ("x", "variable"),
                ("int", "type"),
                ("=", "operator"),
                ("1", "number"),
            ]
        );
        assert_eq!(
            typed("for x: int in $xs { }"),
            [
                ("for", "keyword"),
                ("x", "variable"),
                ("int", "type"),
                ("in", "keyword"),
                ("$xs", "variable"),
            ]
        );
    }

    #[test]
    fn dollar_words_and_rest_patterns_are_variables() {
        assert_eq!(
            typed("match $a { [$b, ..$c] => $env }"),
            [
                ("match", "keyword"),
                ("$a", "variable"),
                ("$b", "variable"),
                ("..$c", "variable"),
                ("=>", "operator"),
                ("$env", "variable"),
            ]
        );
    }

    #[test]
    fn parameters_and_flags_of_signatures_and_closures_are_typed() {
        assert_eq!(
            typed("def f [x?: int = 3, --y (-z): string, ...rest] { }"),
            [
                ("def", "keyword"),
                ("f", "function"),
                ("x?", "parameter"),
                ("int", "type"),
                ("=", "operator"),
                ("3", "number"),
                ("--y", "flag"),
                ("(-z)", "flag"),
                ("string", "type"),
                ("...rest", "parameter"),
            ]
        );
        assert_eq!(
            typed("each {|e| $e }"),
            [("each", "function"), ("e", "parameter"), ("$e", "variable")]
        );
    }

    #[test]
    fn leading_dash_marks_flag_only_in_call() {
        assert_eq!(
            typed("ls -la --all=true"),
            [("ls", "function"), ("-la", "flag"), ("--all=true", "flag")]
        );
        assert_eq!(
            typed("^git -v --x=1"),
            [("^git", "function"), ("-v", "flag"), ("--x=1", "flag")]
        );
        assert_eq!(typed("ls -"), [("ls", "function")]);
        assert_eq!(typed("[-a]"), []);
    }

    #[test]
    fn numbers_and_constants_are_typed() {
        assert_eq!(
            typed("[1 -1 0x[ff] 10kb true null 2024-01-01 1Sec]"),
            [
                ("1", "number"),
                ("-1", "number"),
                ("0x", "number"),
                ("ff", "number"),
                ("10kb", "number"),
                ("true", "keyword"),
                ("null", "keyword"),
                ("2024-01-01", "number"),
            ]
        );
    }

    #[test]
    fn spreads_and_redirections_are_operators() {
        assert_eq!(typed("[...$a]"), [("...", "operator"), ("$a", "variable")]);
        for redirection in ["o>", "o>>", "e>", "e>>", "o+e>", "o+e>>", "e>|", "o+e>|"] {
            assert!(
                typed(&format!("ls {redirection} f")).contains(&(redirection, "operator")),
                "{redirection}"
            );
        }
    }

    #[test]
    fn operators_are_typed_but_not_pipes_around_closure_parameters() {
        assert_eq!(
            typed("1..2 | each {|x| $x + 1 }"),
            [
                ("1", "number"),
                ("..", "operator"),
                ("2", "number"),
                ("|", "operator"),
                ("each", "function"),
                ("x", "parameter"),
                ("$x", "variable"),
                ("+", "operator"),
                ("1", "number"),
            ]
        );
    }

    #[test]
    fn types_of_signatures_are_typed() {
        assert_eq!(
            typed("def f [r: record<a: int>]: nothing -> list<string> { }"),
            [
                ("def", "keyword"),
                ("f", "function"),
                ("r", "parameter"),
                ("record", "type"),
                ("a", "property"),
                ("int", "type"),
                ("nothing", "type"),
                ("->", "operator"),
                ("list", "type"),
                ("string", "type"),
            ]
        );
    }

    #[test]
    fn members_of_cell_paths_and_keys_of_records_are_properties() {
        assert_eq!(
            typed("$a.b.0?"),
            [("$a", "variable"), ("b", "property"), ("0", "property")]
        );
        assert_eq!(
            typed("{a: 1, \"b\": c}"),
            [("a", "property"), ("1", "number"), ("\"b\"", "string")]
        );
    }

    #[test]
    fn keys_of_records_that_nu_evaluates_are_no_properties() {
        assert_eq!(
            typed("{$a: 1, (\"b\"): 2}"),
            [("$a", "variable"), ("1", "number"), ("2", "number")]
        );
    }

    #[test]
    fn arguments_of_unknown_command_take_only_types_of_their_text() {
        assert_eq!(
            typed("my-cmd {|x| $x } $y -f"),
            [("my-cmd", "function"), ("$y", "variable"), ("-f", "flag")]
        );
    }
}
