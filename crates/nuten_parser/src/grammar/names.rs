use std::{cmp::Reverse, iter, rc::Rc};

use nuten_syntax::SyntaxKind;
use rowan::TextSize;

use super::{builtin, is_pipe};
use crate::lexer::Token;

/// What the parser of every level of brackets knows of the names in the file.
#[derive(Clone)]
pub(crate) struct Names<'a> {
    file: Rc<FileNames<'a>>,
    /// The innermost block around the tokens that declares or imports anything.
    /// `None` until the parser enters such a block.
    scope: Option<Rc<Scope<'a>>>,
}

/// What the file names, whatever block a name stands in.
struct FileNames<'a> {
    /// The names the file aliases, sorted so that `is_aliased` can bisect them.
    aliases: Box<[&'a str]>,
    /// The words the file hides, sorted so that `is_hidden` can bisect them.
    /// `None` when it hides an overlay, which may take any command away.
    hidden: Option<Box<[&'a str]>>,
}

/// What a call runs, as far as the blocks around it tell.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Callee {
    /// A command a block around it defines.
    Defined,
    /// One an import or a `hide` may put in place of what a block defines or nu has.
    Unknowable,
    /// No command a block around it defines, and none an import may bring.
    Undefined,
}

impl<'a> Names<'a> {
    pub(crate) fn of(src: &'a str) -> Names<'a> {
        Names {
            file: Rc::new(FileNames {
                aliases: alias_names(src),
                hidden: hidden_words(src),
            }),
            scope: None,
        }
    }

    pub(crate) fn is_aliased(&self, name: &str) -> bool {
        self.file.aliases.binary_search(&name).is_ok()
    }

    /// These names, inside the block of `tokens` too.
    /// `src` is the text `tokens` range over.
    /// `skips_newline` is whether a line break there is no more than whitespace.
    pub(crate) fn within(&self, src: &'a str, tokens: &[Token], skips_newline: bool) -> Names<'a> {
        let lexemes: Vec<_> = tokens
            .iter()
            .filter(|token| !matches!(token.kind, SyntaxKind::Whitespace | SyntaxKind::Comment))
            .map(|token| Lexeme {
                kind: token.kind,
                text: &src[token.range],
                start: token.range.start(),
            })
            .collect();
        let scope = Scope::of(&lexemes, skips_newline, self.scope.clone());
        Names {
            file: Rc::clone(&self.file),
            scope: if scope.definitions.is_empty() && scope.imports.is_empty() {
                self.scope.clone()
            } else {
                Some(Rc::new(scope))
            },
        }
    }

    /// What the call with its head at `head` runs, as far as the blocks around it tell.
    /// `word` gives the text of each item of the call, `0` being the head.
    ///
    /// Nu runs the command of the longest name the items spell.
    /// It looks the name up in the innermost block that declares it.
    /// What a block imports takes the place of a name that block declares from there on.
    /// It takes the place of one an outer block declares as well.
    /// What an import holds is unknowable here, so it is taken to hold every name, however long.
    /// Nu rejects a keyword as the name of a command, so no import brings one of those.
    pub(crate) fn callee<'w>(
        &self,
        head: TextSize,
        word: impl Fn(usize) -> Option<&'w str>,
    ) -> Callee {
        let scopes = || iter::successors(self.scope.as_deref(), |scope| scope.outer.as_deref());
        let longest_name = scopes().map(|scope| scope.longest_name).max().unwrap_or(0);
        let spelled: Vec<_> = (0..longest_name).map_while(&word).collect();
        let Some((depth, scope, definition)) = scopes()
            .enumerate()
            .filter_map(|(depth, scope)| {
                scope
                    .longest_spelled(&spelled)
                    .map(|definition| (depth, scope, definition))
            })
            .max_by_key(|&(depth, _, definition)| (definition.words.len(), Reverse(depth)))
        else {
            let imported = scopes().any(|scope| scope.imports_before(head))
                && word(0).is_some_and(|head| !builtin::is_parser_keyword(head));
            return if imported {
                Callee::Unknowable
            } else {
                Callee::Undefined
            };
        };
        let longer_imported = word(definition.words.len()).is_some()
            && scopes().any(|scope| scope.imports_before(head));
        let replaceable = self.is_hidden(definition)
            || scopes().take(depth).any(|scope| scope.imports_before(head))
            || scope.imports_between(definition.start, head)
            || longer_imported;
        if replaceable {
            Callee::Unknowable
        } else {
            Callee::Defined
        }
    }

    fn is_hidden(&self, definition: &Definition<'_>) -> bool {
        self.file
            .hidden
            .as_ref()
            .is_none_or(|hidden| hidden.binary_search(&definition.words[0]).is_ok())
    }
}

/// A token of a block other than whitespace or a comment.
#[derive(Clone, Copy)]
struct Lexeme<'a> {
    kind: SyntaxKind,
    text: &'a str,
    start: TextSize,
}

/// A block of statements, with what nu declares before it reads them and what they import.
struct Scope<'a> {
    /// Sorted by name, so that `longest_spelled` can bisect them.
    definitions: Box<[Definition<'a>]>,
    /// How many words the longest name among `definitions` has.
    longest_name: usize,
    /// Where each statement that imports names starts, in order.
    imports: Box<[TextSize]>,
    outer: Option<Rc<Scope<'a>>>,
}

/// A command a `def` or an `extern` defines, as nu declares it before it reads the block.
struct Definition<'a> {
    /// The words of the name, which a call spells out one item each.
    words: Vec<&'a str>,
    /// Where the statement starts; nu takes it over a name imported before there.
    start: TextSize,
}

impl<'a> Scope<'a> {
    fn of(lexemes: &[Lexeme<'a>], skips_newline: bool, outer: Option<Rc<Scope<'a>>>) -> Scope<'a> {
        // Nu declares from the statements it reads, where a line break skipped here is whitespace.
        let statement_lexemes: Vec<_> = lexemes
            .iter()
            .copied()
            .filter(|lexeme| !(skips_newline && lexeme.kind == SyntaxKind::Newline))
            .collect();
        let statements = statements(&statement_lexemes);
        let mut definitions: Vec<_> = (0..statements.len())
            .filter(|&nth| stands_alone(&statements, nth))
            .filter_map(|nth| definition(statements[nth]))
            .collect();
        definitions.sort_by(|one, other| one.words.cmp(&other.words));
        Scope {
            longest_name: definitions
                .iter()
                .map(|definition| definition.words.len())
                .max()
                .unwrap_or(0),
            definitions: definitions.into(),
            imports: imports(lexemes).into(),
            outer,
        }
    }

    /// The command of the longest name `spelled` starts with, among those this block defines.
    fn longest_spelled(&self, spelled: &[&str]) -> Option<&Definition<'a>> {
        (1..=spelled.len().min(self.longest_name))
            .rev()
            .find_map(|len| {
                self.definitions
                    .binary_search_by(|definition| definition.words.as_slice().cmp(&spelled[..len]))
                    .ok()
                    .map(|nth| &self.definitions[nth])
            })
    }

    fn imports_before(&self, end: TextSize) -> bool {
        self.imports.first().is_some_and(|&import| import < end)
    }

    fn imports_between(&self, start: TextSize, end: TextSize) -> bool {
        let after = self.imports.partition_point(|&import| import <= start);
        self.imports.get(after).is_some_and(|&import| import < end)
    }
}

/// The statements of `lexemes`, each with at least one lexeme.
fn statements<'l, 'a>(lexemes: &'l [Lexeme<'a>]) -> Vec<&'l [Lexeme<'a>]> {
    lexemes
        .split(|lexeme| matches!(lexeme.kind, SyntaxKind::Semicolon | SyntaxKind::Newline))
        .filter(|statement| !statement.is_empty())
        .collect()
}

/// Whether the statement at `nth` is a pipeline of one command, which is all nu declares from.
/// A pipe first or last joins it to the statement next to it.
fn stands_alone(statements: &[&[Lexeme<'_>]], nth: usize) -> bool {
    let joins =
        |lexeme: Option<&Lexeme<'_>>| lexeme.is_some_and(|lexeme| joins_commands(lexeme.kind));
    !statements[nth]
        .iter()
        .any(|lexeme| joins_commands(lexeme.kind))
        && !nth
            .checked_sub(1)
            .is_some_and(|before| joins(statements[before].last()))
        && !joins(statements.get(nth + 1).and_then(|next| next.first()))
}

/// Whether a token of `kind` keeps a statement from standing alone.
/// `||` is no pipe, but nu declares nothing from a statement with one either.
fn joins_commands(kind: SyntaxKind) -> bool {
    is_pipe(kind) || kind == SyntaxKind::PipePipe
}

/// What the statement defines, as nu's `parse_def_predecl` reads it.
/// `None` where nu declares nothing, or where nuten cannot tell the name nu reads.
fn definition<'a>(statement: &[Lexeme<'a>]) -> Option<Definition<'a>> {
    let start = statement.first()?.start;
    let mut lexemes = statement.iter();
    if !matches!(
        unexported(lexemes.by_ref().map(|lexeme| lexeme.text)),
        Some("def" | "extern")
    ) {
        return None;
    }
    let mut rest = lexemes.skip_while(|lexeme| lexeme.text.starts_with('-'));
    let name = rest.next().filter(|lexeme| lexeme.kind.is_item())?;
    if !rest.any(|lexeme| lexeme.text.starts_with(['[', '('])) {
        return None;
    }
    Some(Definition {
        words: name_words(name.text)?,
        start,
    })
}

/// The words of the name `text` spells, as nu's `parse_string` reads it.
/// `None` for a name nu rejects, and for one nu reads as a keyword whatever the file defines.
/// Also for one that no call spells out.
/// A `=` in one makes nu read the call as setting an environment variable.
fn name_words(text: &str) -> Option<Vec<&str>> {
    let name = unquoted(text)?;
    if name.contains(['#', '^', '%', '='])
        || name.starts_with(|char: char| char.is_ascii_digit() || matches!(char, '.' | '+'))
        || name.parse::<f64>().is_ok()
        || builtin::is_parser_keyword(name)
    {
        return None;
    }
    let words: Vec<_> = name.split(' ').collect();
    if words
        .iter()
        .any(|word| word.is_empty() || word.contains(char::is_whitespace))
        || builtin::reads_before_assignment(words[0], words.get(1).copied())
    {
        return None;
    }
    Some(words)
}

/// The text the string `text` spells, quoted or bare.
/// `None` for one with an escape, which is not unescaped here.
/// Also for one nu reads as more than a string.
fn unquoted(text: &str) -> Option<&str> {
    let quoted = |quote: char| {
        text.strip_prefix(quote)
            .and_then(|rest| rest.strip_suffix(quote))
    };
    match text.chars().next()? {
        '"' => quoted('"').filter(|inner| !inner.contains('\\')),
        '\'' => quoted('\''),
        '`' => quoted('`'),
        _ => (!text.contains(['"', '\'', '`', '\\', '$', '(', '[', '{'])).then_some(text),
    }
}

/// Where each statement that imports names starts, in order.
/// Those are a `use`, a `source`, a `source-env`, and an `overlay use`, exported or not.
/// Also where one follows an assignment operator, which nu reads as statements of the block.
/// A line break counts even where the parser skips it, since nu reads such statements with it.
fn imports(lexemes: &[Lexeme<'_>]) -> Vec<TextSize> {
    (0..lexemes.len())
        .filter(|&nth| {
            nth.checked_sub(1).is_none_or(|before| {
                matches!(
                    lexemes[before].kind,
                    SyntaxKind::Semicolon | SyntaxKind::Newline | SyntaxKind::Assign
                )
            })
        })
        .filter(|&nth| imports_names(lexemes[nth..].iter().map(|lexeme| lexeme.text)))
        .map(|nth| lexemes[nth].start)
        .collect()
}

/// Whether a statement of these words imports names.
fn imports_names<'a>(mut words: impl Iterator<Item = &'a str>) -> bool {
    match unexported(words.by_ref()) {
        Some("use" | "source" | "source-env") => true,
        Some("overlay") => words.next() == Some("use"),
        _ => false,
    }
}

/// The first of `words`, or the one after an `export` first.
fn unexported<'a>(mut words: impl Iterator<Item = &'a str>) -> Option<&'a str> {
    words
        .next()
        .filter(|&word| word != "export")
        .or_else(|| words.next())
}

/// The names `src` aliases, sorted and without repeats.
/// Reads the text, not the tree, which the parser has yet to build.
/// Taking in a name out of scope only leaves a call read as it was written.
fn alias_names(src: &str) -> Box<[&str]> {
    let mut names: Vec<_> = src.split(breaks_statement).filter_map(alias_name).collect();
    names.sort_unstable();
    names.dedup();
    names.into()
}

/// The words after each `hide` in `src`, sorted and without repeats.
/// `None` when `src` hides an overlay.
/// Reads the text, as `alias_names` does.
/// A `hide` may follow an assignment operator, so one anywhere in a statement counts.
/// Nu hides a name even outside the block of the `hide`.
/// Taking in more words than it hides only leaves a call read as it was written.
fn hidden_words(src: &str) -> Option<Box<[&str]>> {
    let statements = || src.split(breaks_statement).map(str::split_ascii_whitespace);
    if statements().any(|words| {
        words
            .clone()
            .zip(words.skip(1))
            .any(|pair| pair == ("overlay", "hide"))
    }) {
        return None;
    }
    let mut words: Vec<_> = statements()
        .flat_map(|words| words.skip_while(|&word| word != "hide").skip(1))
        .map(|word| word.trim_matches(HIDDEN_TRIM))
        .collect();
    words.sort_unstable();
    words.dedup();
    Some(words.into())
}

/// What stands around a word after `hide`, which may name members in a list.
const HIDDEN_TRIM: [char; 6] = ['"', '\'', '`', '[', ']', ','];

/// What nu may start a statement after, so that an `alias` first in one is a head.
fn breaks_statement(char: char) -> bool {
    matches!(char, '\n' | '\r' | ';' | '|' | '{' | '}' | '(' | ')')
}

/// What nu may put around a name.
const NAME_QUOTES: [char; 3] = ['"', '\'', '`'];

fn alias_name(statement: &str) -> Option<&str> {
    let mut words = statement.split_ascii_whitespace();
    if unexported(words.by_ref())? != "alias" {
        return None;
    }
    Some(words.next()?.trim_matches(NAME_QUOTES))
}

#[cfg(test)]
mod tests {
    use rowan::{TextRange, TextSize};

    use super::{Callee, Names, alias_names};
    use crate::lexer::{self, Mode};

    /// What the last line of `src` runs, as a call of its words in the block `src` makes.
    fn callee(src: &str) -> Callee {
        callee_on(src, src.lines().count() - 1)
    }

    /// What the line at `line` of `src` runs, as `callee` tells of the last one.
    fn callee_on(src: &str, line: usize) -> Callee {
        callee_lexed_in(src, line, Mode::Block)
    }

    /// As `callee_on`, with `src` lexed in `mode`, as what brackets of that mode hold.
    fn callee_lexed_in(src: &str, line: usize, mode: Mode) -> Callee {
        let lexed = lexer::lex(src, TextRange::up_to(TextSize::of(src)), mode);
        let start: usize = src.lines().take(line).map(|text| text.len() + 1).sum();
        let words: Vec<_> = src.lines().nth(line).unwrap().split_whitespace().collect();
        Names::of(src)
            .within(src, &lexed.tokens, mode.skips_newline())
            .callee(TextSize::try_from(start).unwrap(), |nth| {
                words.get(nth).copied()
            })
    }

    /// What the last line of `inner` runs, with `inner` as a block inside that of `outer`.
    fn callee_nested(outer: &str, inner: &str) -> Callee {
        let src = format!("{outer}\n{inner}");
        let lex = |range| lexer::lex(&src, range, Mode::Block).tokens;
        let inner_start = TextSize::of(outer) + TextSize::of("\n");
        let outer_tokens = lex(TextRange::up_to(TextSize::of(outer)));
        let inner_tokens = lex(TextRange::new(inner_start, TextSize::of(src.as_str())));
        let last_line_start = src.rfind('\n').unwrap() + 1;
        let words: Vec<_> = src[last_line_start..].split_whitespace().collect();
        Names::of(&src)
            .within(&src, &outer_tokens, false)
            .within(&src, &inner_tokens, false)
            .callee(TextSize::try_from(last_line_start).unwrap(), |nth| {
                words.get(nth).copied()
            })
    }

    #[test]
    fn def_in_outer_block_defines_command_in_inner_block() {
        assert_eq!(callee_nested("def f [] {}", "f"), Callee::Defined);
    }

    #[test]
    fn import_in_inner_block_makes_outer_command_unknowable() {
        assert_eq!(
            callee_nested("def f [] {}", "use m.nu *\nf"),
            Callee::Unknowable
        );
    }

    #[test]
    fn inner_def_takes_place_of_outer_one_of_same_name() {
        assert_eq!(
            callee_nested("def f [] {}\nuse m.nu *", "def f [] {}\nf"),
            Callee::Defined
        );
    }

    #[test]
    fn def_and_extern_define_command() {
        for src in [
            "def f [] {}\nf",
            "export def f [] {}\nf",
            "extern f []\nf",
            "export extern f []\nf",
            "def --env --wrapped f [] {}\nf",
            "def f (x) {}\nf",
            "def f []: any -> any {}\nf",
        ] {
            assert_eq!(callee(src), Callee::Defined, "{src:?}");
        }
    }

    #[test]
    fn call_before_def_in_same_block_runs_it() {
        assert_eq!(callee("f\ndef f [] {}\nf"), Callee::Defined);
        assert_eq!(callee("print a; def f [] {}\nf"), Callee::Defined);
    }

    #[test]
    fn quoted_name_defines_command_of_what_quotes_hold() {
        for src in [
            "def \"f g\" [] {}\nf g",
            "def 'f g' [] {}\nf g",
            "def `f g` [] {}\nf g",
        ] {
            assert_eq!(callee(src), Callee::Defined, "{src:?}");
        }
    }

    #[test]
    fn call_runs_command_only_with_every_word_of_its_name() {
        assert_eq!(callee("def \"f g\" [] {}\nf"), Callee::Undefined);
        assert_eq!(callee("def \"f g\" [] {}\nf h"), Callee::Undefined);
        assert_eq!(callee("def \"f g\" [] {}\nf g h"), Callee::Defined);
        assert_eq!(callee("def f [] {}\ng"), Callee::Undefined);
    }

    #[test]
    fn def_without_signature_defines_nothing() {
        assert_eq!(callee("def f {}\nf"), Callee::Undefined);
        assert_eq!(callee("def f\nf"), Callee::Undefined);
    }

    #[test]
    fn def_in_pipeline_of_several_commands_defines_nothing() {
        for src in [
            "ls | def f [] {}\nf",
            "def f [] {} | ignore\nf",
            "def f [] {} e>| ignore\nf",
            "def f [] {} || true\nf",
            "ls |\ndef f [] {}\nf",
            "ls | # c\ndef f [] {}\nf",
            "def f [] {}\n| ignore\nf",
        ] {
            assert_eq!(callee(src), Callee::Undefined, "{src:?}");
        }
    }

    #[test]
    fn line_break_that_is_whitespace_neither_ends_nor_starts_statement() {
        let src = "\ndef f [] {};\nf";
        assert_eq!(callee_lexed_in(src, 2, Mode::Paren), Callee::Defined);
        let src = "ls\ndef f [] {};\nf";
        assert_eq!(callee_lexed_in(src, 2, Mode::Paren), Callee::Undefined);
    }

    #[test]
    fn def_after_head_of_statement_defines_nothing() {
        assert_eq!(callee("let x = def f [] {}\nf"), Callee::Undefined);
        assert_eq!(callee("@a def f [] {}\nf"), Callee::Undefined);
    }

    #[test]
    fn def_after_attribute_line_defines_command() {
        assert_eq!(
            callee("@example \"e\" { f }\ndef f [] {}\nf"),
            Callee::Defined
        );
    }

    #[test]
    fn name_nu_rejects_defines_nothing() {
        for src in [
            "def if [] {}\nif",
            "def \"export def\" [] {}\nexport def",
            "def \"plugin use\" [] {}\nplugin use",
            "def a#b [] {}\na#b",
            "def 1kb [] {}\n1kb",
            "def inf [] {}\ninf",
        ] {
            assert_eq!(callee(src), Callee::Undefined, "{src:?}");
        }
    }

    #[test]
    fn name_nu_reads_as_keyword_whatever_file_defines_defines_nothing() {
        for src in [
            "def \"for x\" [] {}\nfor x",
            "def \"where x\" [] {}\nwhere x",
            "def \"use x\" [] {}\nuse x",
            "def \"plugin use x\" [] {}\nplugin use x",
        ] {
            assert_eq!(callee(src), Callee::Undefined, "{src:?}");
        }
    }

    #[test]
    fn name_after_keyword_nu_looks_up_defines_command() {
        for src in [
            "def \"if x\" [] {}\nif x",
            "def \"return x\" [] {}\nreturn x",
            "def \"plugin x\" [] {}\nplugin x",
            "def else [] {}\nelse",
        ] {
            assert_eq!(callee(src), Callee::Defined, "{src:?}");
        }
    }

    #[test]
    fn name_no_call_spells_out_defines_nothing() {
        for src in [
            "def \"f  g\" [] {}\nf g",
            "def \" f\" [] {}\nf",
            "def \"f\\tg\" [] {}\nf\\tg",
            "def \"f\\\\\" [] {}\nf\\",
            "def \"x=y\" [] {}\nx=y",
        ] {
            assert_eq!(callee(src), Callee::Undefined, "{src:?}");
        }
    }

    #[test]
    fn import_before_def_leaves_command_defined() {
        for keyword in ["use", "export use", "source", "source-env", "overlay use"] {
            let src = format!("{keyword} m.nu *\ndef f [] {{}}\nf");
            assert_eq!(callee(&src), Callee::Defined, "{src:?}");
        }
    }

    #[test]
    fn import_between_def_and_call_makes_command_unknowable() {
        for keyword in ["use", "export use", "source", "source-env", "overlay use"] {
            let src = format!("def f [] {{}}\n{keyword} m.nu *\nf");
            assert_eq!(callee(&src), Callee::Unknowable, "{src:?}");
        }
        assert_eq!(
            callee("def f [] {}\nlet x = use m.nu *\nf"),
            Callee::Unknowable
        );
    }

    #[test]
    fn import_after_call_leaves_command_defined() {
        assert_eq!(callee_on("def f [] {}\nf\nuse m.nu *", 1), Callee::Defined);
    }

    #[test]
    fn import_before_call_makes_command_no_block_defines_unknowable() {
        for keyword in ["use", "export use", "source", "source-env", "overlay use"] {
            let src = format!("{keyword} m.nu *\nls");
            assert_eq!(callee(&src), Callee::Unknowable, "{src:?}");
        }
        assert_eq!(callee("let x = use m.nu *\nnosuch"), Callee::Unknowable);
    }

    #[test]
    fn import_before_keyword_leaves_it_undefined() {
        for src in [
            "use m.nu *\nif",
            "use m.nu *\nexport def",
            "use m.nu *\nwhere",
        ] {
            assert_eq!(callee(src), Callee::Undefined, "{src:?}");
        }
    }

    #[test]
    fn import_after_call_leaves_command_no_block_defines_undefined() {
        assert_eq!(callee_on("ls\nuse m.nu *", 0), Callee::Undefined);
    }

    #[test]
    fn import_inside_block_leaves_call_outside_it_undefined() {
        assert_eq!(callee("do { use m.nu * }\nls"), Callee::Undefined);
    }

    #[test]
    fn import_in_outer_block_makes_command_in_inner_block_unknowable() {
        assert_eq!(callee_nested("use m.nu *", "ls"), Callee::Unknowable);
    }

    #[test]
    fn import_before_call_with_more_items_than_name_makes_command_unknowable() {
        assert_eq!(callee("use m.nu *\ndef f [] {}\nf g"), Callee::Unknowable);
        assert_eq!(callee("def f [] {}\nf g"), Callee::Defined);
    }

    #[test]
    fn call_runs_command_of_longest_name_it_spells() {
        assert_eq!(
            callee("def f [] {}\nuse m.nu *\ndef \"f g\" [] {}\nf g"),
            Callee::Defined
        );
        assert_eq!(
            callee("def \"f g\" [] {}\nuse m.nu *\ndef f [] {}\nf g"),
            Callee::Unknowable
        );
    }

    #[test]
    fn hidden_name_makes_command_unknowable() {
        for src in [
            "def f [] {}\nhide f\nf",
            "def f [] {}\ndo { hide f }\nf",
            "def f [] {}\nmut v = 1; $v = hide f\nf",
            "def \"f g\" [] {}\nhide f [g]\nf g",
        ] {
            assert_eq!(callee(src), Callee::Unknowable, "{src:?}");
        }
    }

    #[test]
    fn hiding_other_name_leaves_command_defined() {
        assert_eq!(callee("def f [] {}\nhide g\nf"), Callee::Defined);
    }

    #[test]
    fn hidden_overlay_makes_every_command_unknowable() {
        assert_eq!(callee("def f [] {}\noverlay hide o\nf"), Callee::Unknowable);
    }

    #[test]
    fn alias_names_come_from_every_statement() {
        assert_eq!(*alias_names("alias a = ^a"), ["a"]);
        assert_eq!(*alias_names("export alias b = ^b"), ["b"]);
        assert_eq!(*alias_names("print hi;alias c = ^c"), ["c"]);
        assert_eq!(*alias_names("print hi\ralias d = ^d"), ["d"]);
        assert_eq!(*alias_names("ls | alias e = ^e"), ["e"]);
        assert_eq!(*alias_names("do { alias f = ^f }"), ["f"]);
        assert_eq!(*alias_names("(alias g = ^g)"), ["g"]);
    }

    #[test]
    fn alias_names_drop_quotes_around_name() {
        assert_eq!(*alias_names("alias \"a\" = ^a"), ["a"]);
        assert_eq!(*alias_names("alias 'b' = ^b"), ["b"]);
        assert_eq!(*alias_names("alias `c` = ^c"), ["c"]);
    }

    #[test]
    fn alias_names_come_sorted_without_repeats() {
        assert_eq!(
            *alias_names("alias b = ^b\nalias a = ^a\nalias b = ^c"),
            ["a", "b"]
        );
    }

    #[test]
    fn alias_names_leave_out_what_heads_no_statement() {
        assert!(alias_names("# alias a = ^a").is_empty());
        assert!(alias_names("print \"alias b = ^b\"").is_empty());
    }

    #[test]
    fn alias_names_leave_out_statement_that_names_nothing() {
        assert!(alias_names("alias").is_empty());
        assert!(alias_names("export").is_empty());
        assert!(alias_names("export alias").is_empty());
    }
}
