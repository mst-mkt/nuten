use std::{cmp::Reverse, iter, rc::Rc};

use nuten_syntax::SyntaxKind;
use rowan::{TextRange, TextSize};

use super::{
    builtin, interpolation, is_pipe,
    parameters::{Declared, Item, Param},
};
use crate::{
    lexer::{self, Mode, Token},
    parser::MAX_DEPTH,
};

#[derive(Clone)]
pub(crate) struct Names<'a> {
    file: Rc<FileNames<'a>>,
    scope: Option<Rc<Scope<'a>>>,
}

struct FileNames<'a> {
    /// Sorted so that `is_aliased` can bisect them.
    alias_heads: Box<[&'a str]>,
    /// Sorted so that `is_hidden` can bisect them.
    /// `None` when it hides an overlay, which may take any command away.
    hidden: Option<Box<[&'a str]>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Callee {
    Defined,
    Unknowable,
    Undefined,
}

impl<'a> Names<'a> {
    pub(crate) fn of(src: &'a str) -> Names<'a> {
        Names {
            file: Rc::new(FileNames {
                alias_heads: alias_heads(src),
                hidden: hidden_words(src),
            }),
            scope: None,
        }
    }

    pub(crate) fn is_aliased(&self, head: &str) -> bool {
        self.file.alias_heads.binary_search(&head).is_ok()
    }

    /// `skips_newline` is whether a line break there is no more than whitespace.
    pub(crate) fn within(&self, src: &'a str, tokens: &[Token], skips_newline: bool) -> Names<'a> {
        let scope_of = || {
            Scope::of(
                src,
                &lexemes(src, tokens),
                skips_newline,
                self.scope.clone(),
            )
        };
        // Most blocks hold no scope word, and reading their statements takes several allocations.
        let holds_scope_word = tokens
            .iter()
            .any(|token| SCOPE_WORDS.contains(&&src[token.range]));
        debug_assert!(
            holds_scope_word || scope_of().is_empty(),
            "a block with no scope word declares and imports nothing"
        );
        let scope = holds_scope_word
            .then(scope_of)
            .filter(|scope| !scope.is_empty());
        Names {
            file: Rc::clone(&self.file),
            scope: scope.map(Rc::new).or_else(|| self.scope.clone()),
        }
    }

    /// `word` gives the text of each item of the call, `0` being the head.
    /// What an import nuten cannot read holds is unknowable, so it is taken to hold every name.
    /// Nu rejects a keyword as the name of a command, so no import brings one of those.
    pub(crate) fn callee<'w>(
        &self,
        head: TextSize,
        word: impl Fn(usize) -> Option<&'w str>,
    ) -> Callee {
        self.resolved(head, word).0
    }

    pub(crate) fn defined_words<'w>(
        &self,
        head: TextSize,
        word: impl Fn(usize) -> Option<&'w str>,
    ) -> Option<usize> {
        let (Callee::Defined, defining) = self.resolved(head, &word) else {
            return None;
        };
        // Nu runs the longest name it finds.
        // A longer import therefore beats the definition, even from an outer block.
        let spelled: Vec<_> = (0..self.longest_name()).map_while(&word).collect();
        let imported = self
            .scopes()
            .filter_map(|scope| scope.imported_words(&spelled, head))
            .max();
        defining
            .map(|(_, _, definition)| definition.words.len())
            .max(imported)
    }

    fn resolved<'w>(
        &self,
        head: TextSize,
        word: impl Fn(usize) -> Option<&'w str>,
    ) -> (Callee, Option<(usize, &Scope<'a>, &Definition<'a>)>) {
        let spelled: Vec<_> = (0..self.longest_name()).map_while(&word).collect();
        // Nu may take a definition over the alias, but leaving that unknowable only keeps its call.
        if self
            .scopes()
            .any(|scope| scope.imports_alias(&spelled, head))
        {
            return (Callee::Unknowable, None);
        }
        let Some(defining @ (depth, scope, definition)) = self.defining(&spelled) else {
            let unknowable = self.scopes().any(|scope| scope.imports_before(head))
                && word(0).is_some_and(|head| !builtin::is_parser_keyword(head));
            let callee = if unknowable {
                Callee::Unknowable
            } else if !self
                .scopes()
                .any(|scope| scope.imported_words(&spelled, head).is_some())
            {
                Callee::Undefined
            } else if self.is_hidden(spelled[0]) {
                Callee::Unknowable
            } else {
                Callee::Defined
            };
            return (callee, None);
        };
        let longer_imported = word(definition.words.len()).is_some()
            && self.scopes().any(|scope| scope.imports_before(head));
        let replaceable = self.is_hidden(definition.words[0])
            || self
                .scopes()
                .take(depth)
                .any(|scope| scope.imports_before(head))
            || scope.imports_between(definition.start, head)
            || longer_imported;
        if replaceable {
            (Callee::Unknowable, None)
        } else {
            (Callee::Defined, Some(defining))
        }
    }

    /// A name starting with a word that names a command nu has may run that, as nu runs the longer.
    /// Nu brings a definition in at its statement, so only a later import in its block replaces it.
    pub(crate) fn defined_parameters(
        &self,
        head: TextSize,
        items: &[&str],
    ) -> Option<Vec<Option<Param>>> {
        let (_, defining) = self.resolved(head, |nth| items.get(nth).copied());
        let (depth, scope, definition) = defining?;
        let declared = definition.declared.as_ref()?;
        let words = definition.words.len();
        let spelled = &items[..self.longest_name().min(items.len())];
        let imported = |scope: &Scope<'a>, len, from| scope.imports_named(spelled, len, from, head);
        let replaced = builtin::is_command(definition.words[0])
            || self
                .scopes()
                .any(|scope| imported(scope, words + 1, TextSize::new(0)))
            || self
                .scopes()
                .take(depth)
                .any(|scope| imported(scope, words, TextSize::new(0)))
            || imported(scope, words, definition.start);
        if replaced {
            return None;
        }
        let arguments = declared.arguments(&items[words..]);
        Some(iter::repeat_n(None, words).chain(arguments).collect())
    }

    fn scopes(&self) -> impl Iterator<Item = &Scope<'a>> {
        iter::successors(self.scope.as_deref(), |scope| scope.outer.as_deref())
    }

    fn longest_name(&self) -> usize {
        self.scopes()
            .map(|scope| scope.longest_name)
            .max()
            .unwrap_or(0)
    }

    fn defining(&self, spelled: &[&str]) -> Option<(usize, &Scope<'a>, &Definition<'a>)> {
        self.scopes()
            .enumerate()
            .filter_map(|(depth, scope)| {
                scope
                    .longest_spelled(spelled)
                    .map(|definition| (depth, scope, definition))
            })
            .max_by_key(|&(depth, _, definition)| (definition.words.len(), Reverse(depth)))
    }

    fn is_hidden(&self, first_word: &str) -> bool {
        self.file
            .hidden
            .as_ref()
            .is_none_or(|hidden| hidden.binary_search(&first_word).is_ok())
    }
}

#[derive(Clone, Copy)]
struct Lexeme<'a> {
    kind: SyntaxKind,
    text: &'a str,
    start: TextSize,
}

fn lexemes<'a>(src: &'a str, tokens: &[Token]) -> Vec<Lexeme<'a>> {
    tokens
        .iter()
        .filter(|token| !matches!(token.kind, SyntaxKind::Whitespace | SyntaxKind::Comment))
        .map(|token| Lexeme {
            kind: token.kind,
            text: &src[token.range],
            start: token.range.start(),
        })
        .collect()
}

const SCOPE_WORDS: [&str; 6] = ["def", "extern", "module", "use", "source", "source-env"];

struct Scope<'a> {
    /// Sorted by name, so that `longest_spelled` can bisect them.
    definitions: Box<[Definition<'a>]>,
    /// Sorted by name, then by where they start.
    imported: Box<[Definition<'a>]>,
    longest_name: usize,
    imports: Box<[TextSize]>,
    modules: Box<[Module<'a>]>,
    outer: Option<Rc<Scope<'a>>>,
}

struct Definition<'a> {
    words: Vec<&'a str>,
    /// Nu takes the definition over a name imported before its start.
    start: TextSize,
    /// An import of a `def` does not carry its parameters here.
    declared: Option<Declared<'a>>,
    alias: bool,
}

struct Module<'a> {
    name: &'a str,
    start: TextSize,
    exports: Option<Exports<'a>>,
}

struct Exports<'a> {
    commands: Vec<Exported<'a>>,
    /// An import brings `main` as the name of the module, so it stands apart from `commands`.
    main: bool,
}

#[derive(Debug, Clone)]
struct Exported<'a> {
    words: Vec<&'a str>,
    /// Nu runs the command an alias stands for, which nuten does not follow.
    alias: bool,
}

impl<'a> Scope<'a> {
    fn of(
        src: &'a str,
        lexemes: &[Lexeme<'a>],
        skips_newline: bool,
        outer: Option<Rc<Scope<'a>>>,
    ) -> Scope<'a> {
        // Nu declares from the statements it reads, where a line break skipped here is whitespace.
        let statement_lexemes: Vec<_> = lexemes
            .iter()
            .copied()
            .filter(|lexeme| !(skips_newline && lexeme.kind == SyntaxKind::Newline))
            .collect();
        let statements = statements(&statement_lexemes);
        let alone: Vec<_> = (0..statements.len())
            .filter(|&nth| stands_alone(&statements, nth))
            .map(|nth| statements[nth])
            .collect();
        let mut definitions: Vec<_> = alone
            .iter()
            .filter_map(|statement| definition(src, statement))
            .collect();
        definitions.sort_by(|one, other| one.words.cmp(&other.words));
        let modules: Box<[_]> = alone
            .iter()
            .filter_map(|statement| module(src, statement))
            .collect();
        let read = read_imports(src, &alone, &modules, outer.as_deref());
        let mut imported: Vec<_> = read
            .iter()
            .flat_map(|(start, names)| {
                names.iter().map(|exported| Definition {
                    words: exported.words.clone(),
                    start: *start,
                    declared: None,
                    alias: exported.alias,
                })
            })
            .collect();
        imported.sort_by(|one, other| (&one.words, one.start).cmp(&(&other.words, other.start)));
        let imports: Box<[_]> = imports(&statement_lexemes)
            .into_iter()
            .filter(|import| !read.iter().any(|(start, _)| start == import))
            .collect();
        Scope {
            longest_name: definitions
                .iter()
                .chain(&imported)
                .map(|definition| definition.words.len())
                .max()
                .unwrap_or(0),
            definitions: definitions.into(),
            imported: imported.into(),
            imports,
            modules,
            outer,
        }
    }

    fn is_empty(&self) -> bool {
        self.definitions.is_empty()
            && self.imported.is_empty()
            && self.imports.is_empty()
            && self.modules.is_empty()
    }

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

    fn imported_words(&self, spelled: &[&str], end: TextSize) -> Option<usize> {
        (1..=spelled.len().min(self.longest_name))
            .rev()
            .find(|&len| {
                let first = self
                    .imported
                    .partition_point(|command| command.words.as_slice() < &spelled[..len]);
                self.imported
                    .get(first)
                    .is_some_and(|command| command.words == spelled[..len] && command.start < end)
            })
    }

    fn imports_named(&self, spelled: &[&str], len: usize, from: TextSize, end: TextSize) -> bool {
        self.imported.iter().any(|command| {
            command.words.len() >= len
                && spelled.starts_with(&command.words)
                && (from..end).contains(&command.start)
        })
    }

    fn imports_alias(&self, spelled: &[&str], end: TextSize) -> bool {
        self.imported.iter().any(|command| {
            command.alias && spelled.starts_with(&command.words) && command.start < end
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

/// `statements` must be those of a block that stand alone, whose modules are `modules`.
fn read_imports<'a>(
    src: &'a str,
    statements: &[&[Lexeme<'a>]],
    modules: &[Module<'a>],
    outer: Option<&Scope<'a>>,
) -> Vec<(TextSize, Vec<Exported<'a>>)> {
    statements
        .iter()
        .flat_map(|statement| assigned_parts(statement))
        .filter_map(|part| {
            let start = part.first()?.start;
            let module = |name: &str| visible_module(modules, outer, name, start);
            imported_names(src, part, module).map(|names| (start, names))
        })
        .collect()
}

/// Nu looks in the innermost block first, and takes the last module declared before the import.
/// `modules` are those of the block of the import, and `outer` the scope around it.
fn visible_module<'s, 'a>(
    modules: &'s [Module<'a>],
    outer: Option<&'s Scope<'a>>,
    name: &str,
    start: TextSize,
) -> Option<&'s Module<'a>> {
    let declared = |modules: &'s [Module<'a>]| {
        modules
            .iter()
            .rev()
            .find(|module| module.name == name && module.start < start)
    };
    declared(modules).or_else(|| {
        iter::successors(outer, |scope| scope.outer.as_deref())
            .find_map(|scope| declared(&scope.modules))
    })
}

fn statements<'l, 'a>(lexemes: &'l [Lexeme<'a>]) -> Vec<&'l [Lexeme<'a>]> {
    lexemes
        .split(|lexeme| matches!(lexeme.kind, SyntaxKind::Semicolon | SyntaxKind::Newline))
        .filter(|statement| !statement.is_empty())
        .collect()
}

/// Nu declares only from a statement that is a pipeline of one command.
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

/// `||` is no pipe, but nu declares nothing from a statement with one either.
fn joins_commands(kind: SyntaxKind) -> bool {
    is_pipe(kind) || kind == SyntaxKind::PipePipe
}

/// Reads the statement as nu's `parse_def_predecl` does.
fn definition<'a>(src: &'a str, statement: &[Lexeme<'a>]) -> Option<Definition<'a>> {
    let start = statement.first()?.start;
    let mut lexemes = statement.iter();
    let keyword = unexported(lexemes.by_ref().map(|lexeme| lexeme.text))
        .filter(|keyword| matches!(*keyword, "def" | "extern"))?;
    let rest: Vec<_> = lexemes.collect();
    let flags = rest
        .iter()
        .take_while(|lexeme| lexeme.text.starts_with('-'))
        .count();
    let name = rest.get(flags).filter(|lexeme| lexeme.kind.is_item())?;
    let after_name = &rest[flags + 1..];
    if !after_name
        .iter()
        .any(|lexeme| lexeme.text.starts_with(['[', '(']))
    {
        return None;
    }
    // A wrapped `def` and an `extern` take arguments their signature does not declare.
    let wrapped = rest[..flags]
        .iter()
        .any(|lexeme| lexeme.text == "--wrapped");
    let declared = after_name
        .first()
        .filter(|_| keyword == "def" && !wrapped)
        .and_then(|signature| declared(src, signature));
    Some(Definition {
        words: name_words(name.text)?,
        start,
        declared,
        alias: false,
    })
}

fn declared<'a>(src: &'a str, lexeme: &Lexeme<'a>) -> Option<Declared<'a>> {
    let text = lexeme.text.strip_suffix(':').unwrap_or(lexeme.text);
    let tokens = [('[', ']'), ('(', ')')].into_iter().find_map(|brackets| {
        bracketed_tokens(src, text, lexeme.start, brackets, Mode::Signature)
    })?;
    Declared::of(tokens.iter().map(|token| match token.kind {
        SyntaxKind::Word => Item::Word(&src[token.range]),
        SyntaxKind::Colon => Item::Colon,
        SyntaxKind::Eq => Item::Eq,
        kind if kind.is_item() => Item::Value,
        _ => Item::Other,
    }))
}

/// Reads the name as nu's `parse_string` does.
/// A `=` in a name makes nu read the call as setting an environment variable.
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

/// Nu reads what follows each assignment operator as a statement of the block.
fn assigned_parts<'l, 'a>(statement: &'l [Lexeme<'a>]) -> impl Iterator<Item = &'l [Lexeme<'a>]> {
    iter::once(statement).chain(
        statement
            .iter()
            .enumerate()
            .filter(|(_, lexeme)| lexeme.kind == SyntaxKind::Assign)
            .map(|(nth, _)| &statement[nth + 1..]),
    )
}

/// A module with no body names a file nuten does not read.
fn module<'a>(src: &'a str, statement: &[Lexeme<'a>]) -> Option<Module<'a>> {
    let words = match statement {
        [keyword, rest @ ..] if keyword.text == "module" => rest,
        [export, keyword, rest @ ..] if export.text == "export" && keyword.text == "module" => rest,
        _ => return None,
    };
    let [name, body] = words else {
        return None;
    };
    let inside = body.text.strip_prefix('{')?.strip_suffix('}')?;
    let name = unquoted(name.text)
        .filter(|name| name_words(name).is_some_and(|words| words.len() == 1))?;
    let inside = TextRange::at(body.start + TextSize::of('{'), TextSize::of(inside));
    Some(Module {
        name,
        start: statement[0].start,
        exports: exports(src, inside, name),
    })
}

/// `inside` is where the text inside its braces stands in `src`.
fn exports<'a>(src: &'a str, inside: TextRange, module: &str) -> Option<Exports<'a>> {
    let lexed = lexer::lex(src, inside, Mode::Block);
    if !lexed.errors.is_empty() {
        return None;
    }
    let lexemes = lexemes(src, &lexed.tokens);
    let statements = statements(&lexemes);
    let names = (0..statements.len())
        .map(|nth| exported(src, &statements, nth))
        .filter(|export| !matches!(export, Export::Nothing))
        .map(|export| match export {
            Export::Command(exported) => Some(exported),
            Export::Nothing | Export::Unreadable => None,
        })
        .collect::<Option<Vec<_>>>()?;
    // Nu takes an export named after its module for its `main`, or rejects it as an alias.
    // Nuten reads neither.
    if names
        .iter()
        .any(|exported| exported.words.as_slice() == [module])
    {
        return None;
    }
    let (main, commands): (Vec<_>, Vec<_>) = names
        .into_iter()
        .partition(|exported| exported.words.as_slice() == ["main"]);
    Some(Exports {
        commands,
        main: !main.is_empty(),
    })
}

#[derive(Debug)]
enum Export<'a> {
    Command(Exported<'a>),
    Nothing,
    Unreadable,
}

fn exported<'a>(src: &'a str, statements: &[&[Lexeme<'a>]], nth: usize) -> Export<'a> {
    let statement = statements[nth];
    let command = match statement {
        [export, keyword, ..] if export.text == "export" => match keyword.text {
            "def" | "extern" if stands_alone(statements, nth) => {
                definition(src, statement).map(|definition| Exported {
                    words: definition.words,
                    alias: false,
                })
            },
            "alias" if stands_alone(statements, nth) => statement
                .get(2)
                .and_then(|name| unquoted(name.text))
                .and_then(name_words)
                .map(|words| Exported {
                    words,
                    alias: true,
                }),
            "const" => return Export::Nothing,
            _ => None,
        },
        [export] if export.text == "export" => None,
        _ => return Export::Nothing,
    };
    command.map_or(Export::Unreadable, Export::Command)
}

/// `module` finds the module a name stands for, where a block declares it.
/// Nu rejects a member the module does not export.
/// Nu reads a member `main` apart from the other exports.
fn imported_names<'s, 'a: 's>(
    src: &'a str,
    statement: &[Lexeme<'a>],
    module: impl Fn(&str) -> Option<&'s Module<'a>>,
) -> Option<Vec<Exported<'a>>> {
    let words = match statement {
        [keyword, rest @ ..] if keyword.text == "use" => rest,
        [export, keyword, rest @ ..] if export.text == "export" && keyword.text == "use" => rest,
        _ => return None,
    };
    let (name, members) = match words {
        [name] => (name, None),
        [name, members] => (name, Some(members)),
        _ => return None,
    };
    if !name.kind.is_item() || name.kind == SyntaxKind::Error {
        return None;
    }
    let module = module(unquoted(name.text)?)?;
    let exports = module.exports.as_ref()?;
    // Nu rejects an alias named `main` as an export.
    let main = exports.main.then(|| Exported {
        words: vec![module.name],
        alias: false,
    });
    let Some(members) = members else {
        let prefixed = exports.commands.iter().map(|exported| Exported {
            words: iter::once(module.name)
                .chain(exported.words.iter().copied())
                .collect(),
            alias: exported.alias,
        });
        return Some(main.into_iter().chain(prefixed).collect());
    };
    if !members.kind.is_item() || members.kind == SyntaxKind::Error {
        return None;
    }
    if unquoted(members.text) == Some("*") {
        return Some(
            main.into_iter()
                .chain(exports.commands.iter().cloned())
                .collect(),
        );
    }
    // Nu takes the last export of a name.
    member_names(src, members)?
        .into_iter()
        .map(|member| {
            exports
                .commands
                .iter()
                .rfind(|exported| exported.words.iter().copied().eq(member.split(' ')))
                .cloned()
        })
        .collect()
}

fn member_names<'a>(src: &'a str, members: &Lexeme<'a>) -> Option<Vec<&'a str>> {
    if !members.text.starts_with('[') {
        return Some(vec![unquoted(members.text)?]);
    }
    bracketed_tokens(src, members.text, members.start, ('[', ']'), Mode::List)?
        .iter()
        .map(|token| {
            token
                .kind
                .is_item()
                .then(|| unquoted(&src[token.range]))
                .flatten()
        })
        .collect()
}

/// `text` stands at `start` in `src`.
fn bracketed_tokens(
    src: &str,
    text: &str,
    start: TextSize,
    (open, close): (char, char),
    mode: Mode,
) -> Option<Vec<Token>> {
    let inside = text.strip_prefix(open)?.strip_suffix(close)?;
    let range = TextRange::at(start + TextSize::of(open), TextSize::of(inside));
    let lexed = lexer::lex(src, range, mode);
    lexed.errors.is_empty().then(|| {
        lexed
            .tokens
            .into_iter()
            .filter(|token| !token.kind.is_trivia() && token.kind != SyntaxKind::Comma)
            .collect()
    })
}

/// Nu reads what follows an assignment operator as a statement of the block.
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

fn imports_names<'a>(mut words: impl Iterator<Item = &'a str>) -> bool {
    match unexported(words.by_ref()) {
        Some("use" | "source" | "source-env") => true,
        Some("overlay") => words.next() == Some("use"),
        _ => false,
    }
}

fn unexported<'a>(mut words: impl Iterator<Item = &'a str>) -> Option<&'a str> {
    words
        .next()
        .filter(|&word| word != "export")
        .or_else(|| words.next())
}

/// Reads the tokens, not the tree, which the parser has yet to build.
/// Taking in a name out of scope only leaves a call read as it was written.
/// One in brackets too deep for the parser is left out: it counts only there, kept as written.
/// A call of an alias named in several words starts with its first word.
/// Taking in a call of another command with that word only leaves it read as it was written.
fn alias_heads(src: &str) -> Box<[&str]> {
    let mut heads: Vec<_> = statements_holding(src, "alias")
        .0
        .iter()
        .map(Vec::as_slice)
        .filter_map(alias_name)
        .map(|name| name.split_once(' ').map_or(name, |(head, _)| head))
        .collect();
    heads.sort_unstable();
    heads.dedup();
    heads.into()
}

/// A `hide` may follow an assignment operator, so one anywhere in a statement counts.
/// Nu hides a name even outside the block of the `hide`.
/// Taking in more words than it hides only leaves a call read as it was written.
fn hidden_words(src: &str) -> Option<Box<[&str]>> {
    let (statements, too_deep) = statements_holding(src, "hide");
    if too_deep
        || statements
            .iter()
            .any(|statement| statement.windows(2).any(|pair| pair == ["overlay", "hide"]))
    {
        return None;
    }
    let mut words: Vec<_> = statements
        .iter()
        .flat_map(|statement| {
            statement
                .iter()
                .skip_while(|&&item| item != "hide")
                .skip(1)
                .flat_map(|item| item.split_ascii_whitespace())
        })
        .map(|word| word.trim_matches(HIDDEN_TRIM))
        .collect();
    words.sort_unstable();
    words.dedup();
    Some(words.into())
}

const HIDDEN_TRIM: [char; 6] = ['"', '\'', '`', '[', ']', ','];

/// The lexer splits them, so that strings, comments, and line breaks read as nu reads them.
/// Only brackets that hold `word` are lexed again, which spares most files any lexing.
/// Each level lexes the levels inside it again, so the depth bounds the time.
fn statements_holding<'a>(src: &'a str, word: &str) -> (Vec<Vec<&'a str>>, bool) {
    let mut pending = vec![(TextRange::up_to(TextSize::of(src)), Mode::Block, 0)];
    let mut statements = Vec::new();
    let mut too_deep = false;
    while let Some((range, mode, depth)) = pending.pop() {
        if !src[range].contains(word) {
            continue;
        }
        if depth > MAX_DEPTH {
            too_deep = true;
            continue;
        }
        let tokens = significant_tokens(src, range, mode);
        statements.extend(
            statements_of(src, &tokens, mode).filter(|statement| statement.contains(&word)),
        );
        pending.extend(
            tokens
                .iter()
                .flat_map(|token| insides(src, token, mode))
                .map(|(range, mode)| (range, mode, depth + 1)),
        );
    }
    (statements, too_deep)
}

fn significant_tokens(src: &str, range: TextRange, mode: Mode) -> Vec<Token> {
    lexer::tokens(src, range, mode)
        .filter(|token| {
            !(matches!(token.kind, SyntaxKind::Whitespace | SyntaxKind::Comment)
                || (token.kind == SyntaxKind::Newline && mode.skips_newline()))
        })
        .collect()
}

/// A pipe ends one too, since an `alias` may follow it.
fn statements_of<'a, 't>(
    src: &'a str,
    tokens: &'t [Token],
    mode: Mode,
) -> impl Iterator<Item = Vec<&'a str>> + use<'a, 't> {
    let ends_statement = |token: &Token| {
        matches!(
            token.kind,
            SyntaxKind::Newline
                | SyntaxKind::Semicolon
                | SyntaxKind::Pipe
                | SyntaxKind::PipePipe
                | SyntaxKind::ErrGtPipe
                | SyntaxKind::OutErrGtPipe
        )
    };
    tokens
        .split(ends_statement)
        .filter(move |run| mode != Mode::List && !run.is_empty())
        .map(|run| run.iter().map(|token| &src[token.range]).collect())
}

/// `mode` is the one `token` was lexed in.
/// The parser splits a word such as `a:{|| ls }` or `--flag=(ls)` before it reads its brackets.
fn insides(src: &str, token: &Token, mode: Mode) -> Vec<(TextRange, Mode)> {
    // Every bracket is one byte.
    let inside = |range: TextRange| {
        TextRange::new(
            range.start() + TextSize::from(1),
            range.end() - TextSize::from(1),
        )
    };
    match token.kind {
        SyntaxKind::Word => lexer::item_brackets(src, token.range, mode)
            .into_iter()
            .filter_map(|brackets| {
                let inner = match src.as_bytes()[usize::from(brackets.start())] {
                    b'(' => Mode::Paren,
                    b'{' => Mode::Block,
                    b'[' => Mode::List,
                    _ => return None,
                };
                Some((inside(brackets), inner))
            })
            .collect(),
        SyntaxKind::InterpString => {
            let offset = |at: usize| TextSize::try_from(at).expect("an offset in a token");
            interpolation::parens(&src[token.range])
                .into_iter()
                .map(|paren| {
                    let range = TextRange::new(offset(paren.start), offset(paren.end));
                    (inside(range + token.range.start()), Mode::Paren)
                })
                .collect()
        },
        _ => Vec::new(),
    }
}

const NAME_QUOTES: [char; 3] = ['"', '\'', '`'];

fn alias_name<'a>(statement: &[&'a str]) -> Option<&'a str> {
    let mut items = statement.iter().copied();
    if unexported(items.by_ref())? != "alias" {
        return None;
    }
    Some(items.next()?.trim_matches(NAME_QUOTES))
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use rowan::{TextRange, TextSize};

    use super::{Callee, MAX_DEPTH, Names, alias_heads, hidden_words};
    use crate::{
        grammar::parameters::Takes,
        lexer::{self, Mode},
    };

    fn defined_takes(src: &str) -> Option<Vec<Option<Takes>>> {
        defined_takes_on(src, src.lines().count() - 1)
    }

    fn defined_takes_on(src: &str, line: usize) -> Option<Vec<Option<Takes>>> {
        let lexed = lexer::lex(src, TextRange::up_to(TextSize::of(src)), Mode::Block);
        let start: usize = src.lines().take(line).map(|text| text.len() + 1).sum();
        let words: Vec<_> = src.lines().nth(line)?.split_whitespace().collect();
        let params = Names::of(src)
            .within(src, &lexed.tokens, false)
            .defined_parameters(TextSize::try_from(start).unwrap(), &words)?;
        Some(
            params
                .into_iter()
                .map(|param| param.map(|param| param.takes))
                .collect(),
        )
    }

    #[test]
    fn def_call_takes_arguments_signature_declares() {
        use Takes::{List, Word};
        for src in [
            "def f [x: list, y: string] {}\nf [a] [b]",
            "f [a] [b]\ndef f [x: list, y: string] {}\nf [a] [b]",
            "def f (x: list, y: string) {}\nf [a] [b]",
            "def f [x: list, y: string]: any -> any {}\nf [a] [b]",
            "def --env f [x: list, y: string] {}\nf [a] [b]",
        ] {
            assert_eq!(
                defined_takes(src),
                Some(vec![None, Some(List), Some(Word)]),
                "{src:?}"
            );
        }
    }

    #[test]
    fn extern_and_wrapped_def_leave_arguments_unread() {
        for src in [
            "extern f [x: list]\nf [a]",
            "def --wrapped f [...x] {}\nf [a]",
        ] {
            assert_eq!(defined_takes(src), None, "{src:?}");
        }
    }

    #[test]
    fn builtin_of_longer_name_replaces_def() {
        assert_eq!(defined_takes("def str [x] {}\nstr join [a]"), None);
    }

    #[test]
    fn def_named_after_builtin_leaves_arguments_unread() {
        assert_eq!(defined_takes("def ls [x] {}\nls [a]"), None);
    }

    #[test]
    fn module_import_after_call_leaves_command_defined() {
        let src = "def f [...x] {}\nf g [a]\nmodule m { export def \"f g\" [] {} }\nuse m *";
        assert_eq!(
            defined_takes_on(src, 1),
            Some(vec![None, Some(Takes::List), Some(Takes::List)])
        );
    }

    #[test]
    fn module_import_replaces_earlier_def() {
        let module = "module n { export def f [x: string] {} }";
        for src in [
            format!("{module}\ndef f [x: list] {{}}\nuse n f\nf [a]"),
            format!("{module}\ndef \"n f\" [x: list] {{}}\nuse n\nn f [a]"),
        ] {
            let line = src.lines().position(|line| line.ends_with("[a]")).unwrap();
            assert_eq!(defined_takes_on(&src, line), None, "{src:?}");
        }
    }

    #[test]
    fn inner_module_import_replaces_outer_def() {
        let outer = "module n { export def f [x: string] {} }\ndef f [x: list] {}";
        assert_eq!(defined_takes_nested(outer, "use n f\nf [a]"), None);
        assert_eq!(
            defined_takes_nested(outer, "f [a]"),
            Some(vec![None, Some(Takes::List)])
        );
    }

    #[test]
    fn def_replaces_earlier_module_import() {
        let src = "module n { export def f [x: string] {} }\nuse n f\ndef f [x: list] {}\nf [a]";
        assert_eq!(defined_takes(src), Some(vec![None, Some(Takes::List)]));
    }

    #[test]
    fn module_import_of_longer_name_replaces_def() {
        let src = "module m { export def \"f g\" [] {} }\nuse m *\ndef f [x] {}\nf g [a]";
        assert_eq!(defined_takes(src), None);
        let src = "module m { export def \"f h\" [] {} }\nuse m *\ndef f [x] {}\nf g [a]";
        assert_eq!(
            defined_takes(src),
            Some(vec![None, Some(Takes::List), None])
        );
    }

    #[test]
    fn unknowable_command_leaves_arguments_unread() {
        assert_eq!(defined_takes("def f [x] {}\nuse m.nu *\nf [a]"), None);
    }

    #[test]
    fn undefined_command_leaves_arguments_unread() {
        assert_eq!(defined_takes("nosuch [a]"), None);
    }

    #[test]
    fn unreadable_signature_leaves_arguments_unread() {
        for def in [
            "def f [x: nosuch] {}",
            "def f [x = [1]] {}",
            "def f [x = \"a] {}",
            "def f [...r = 1] {}",
        ] {
            let src = format!("{def}\nf [a]");
            assert_eq!(defined_takes(&src), None, "{src:?}");
        }
    }

    #[test]
    fn parameter_with_string_default_takes_word() {
        let src = "def f [x = \"a\"] {}\nf [a]";
        assert_eq!(defined_takes(src), Some(vec![None, Some(Takes::Word)]));
    }

    fn callee(src: &str) -> Callee {
        callee_on(src, src.lines().count() - 1)
    }

    fn callee_on(src: &str, line: usize) -> Callee {
        callee_lexed_in(src, line, Mode::Block)
    }

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

    fn callee_nested(outer: &str, inner: &str) -> Callee {
        in_nested(outer, inner, |names, head, words| {
            names.callee(head, |nth| words.get(nth).copied())
        })
    }

    fn defined_takes_nested(outer: &str, inner: &str) -> Option<Vec<Option<Takes>>> {
        in_nested(outer, inner, |names, head, words| {
            let params = names.defined_parameters(head, words)?;
            Some(
                params
                    .into_iter()
                    .map(|param| param.map(|param| param.takes))
                    .collect(),
            )
        })
    }

    fn in_nested<T>(
        outer: &str,
        inner: &str,
        read: impl Fn(&Names<'_>, TextSize, &[&str]) -> T,
    ) -> T {
        let src = format!("{outer}\n{inner}");
        let lex = |range| lexer::lex(&src, range, Mode::Block).tokens;
        let inner_start = TextSize::of(outer) + TextSize::of("\n");
        let outer_tokens = lex(TextRange::up_to(TextSize::of(outer)));
        let inner_tokens = lex(TextRange::new(inner_start, TextSize::of(src.as_str())));
        let last_line_start = src.rfind('\n').unwrap() + 1;
        let words: Vec<_> = src[last_line_start..].split_whitespace().collect();
        let names =
            Names::of(&src)
                .within(&src, &outer_tokens, false)
                .within(&src, &inner_tokens, false);
        read(&names, TextSize::try_from(last_line_start).unwrap(), &words)
    }

    #[test]
    fn inner_call_runs_outer_def() {
        assert_eq!(callee_nested("def f [] {}", "f"), Callee::Defined);
    }

    #[test]
    fn inner_import_makes_outer_def_unknowable() {
        assert_eq!(
            callee_nested("def f [] {}", "use m.nu *\nf"),
            Callee::Unknowable
        );
    }

    #[test]
    fn inner_def_replaces_outer_unknowable_command() {
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
    fn def_after_other_statement_defines_command() {
        assert_eq!(callee("f\ndef f [] {}\nf"), Callee::Defined);
        assert_eq!(callee("print a; def f [] {}\nf"), Callee::Defined);
    }

    #[test]
    fn quoted_name_defines_unquoted_command() {
        for src in [
            "def \"f g\" [] {}\nf g",
            "def 'f g' [] {}\nf g",
            "def `f g` [] {}\nf g",
        ] {
            assert_eq!(callee(src), Callee::Defined, "{src:?}");
        }
    }

    #[test]
    fn only_whole_name_runs_command() {
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
    fn def_in_pipeline_defines_nothing() {
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
    fn skipped_line_break_separates_no_statements() {
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
    fn name_read_as_keyword_defines_nothing() {
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
    fn keyword_name_nu_looks_up_defines_command() {
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
    fn earlier_import_makes_undefined_command_unknowable() {
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
    fn import_after_call_leaves_command_undefined() {
        assert_eq!(callee_on("ls\nuse m.nu *", 0), Callee::Undefined);
    }

    #[test]
    fn inner_import_leaves_outer_call_undefined() {
        assert_eq!(callee("do { use m.nu * }\nls"), Callee::Undefined);
    }

    #[test]
    fn outer_import_makes_inner_call_unknowable() {
        assert_eq!(callee_nested("use m.nu *", "ls"), Callee::Unknowable);
    }

    const MODULE: &str = "module m { export def f [] {}; export def g [] {}; def h [] {} }";

    #[test]
    fn module_import_brings_commands_it_names() {
        for (import, call) in [
            ("use m f", "f"),
            ("use m", "m f"),
            ("use m *", "g"),
            ("use m [f g]", "g"),
            ("use m [f, \"g\"]", "g"),
            ("use m \"*\"", "f"),
            ("use \"m\" f", "f"),
            ("export use m f", "f"),
            ("let x = use m f", "f"),
        ] {
            let src = format!("{MODULE}\n{import}\n{call}");
            assert_eq!(callee(&src), Callee::Defined, "{src:?}");
        }
    }

    #[test]
    fn module_import_brings_no_command_it_omits() {
        for (import, call) in [
            ("use m f", "g"),
            ("use m", "f"),
            ("use m *", "h"),
            ("use m []", "f"),
        ] {
            let src = format!("{MODULE}\n{import}\n{call}");
            assert_eq!(callee(&src), Callee::Undefined, "{src:?}");
        }
    }

    #[test]
    fn module_import_renames_main_to_module() {
        let module = "module m { export def main [] {}; export def f [] {} }";
        for import in ["use m", "use m *"] {
            let src = format!("{module}\n{import}\nm");
            assert_eq!(callee(&src), Callee::Defined, "{src:?}");
        }
    }

    #[test]
    fn unreadable_module_import_makes_command_unknowable() {
        for (module, import) in [
            (MODULE, "use m main"),
            (MODULE, "use m nosuch"),
            (MODULE, "use m [f nosuch]"),
            (MODULE, "use m f g"),
            (MODULE, "use --help m f"),
            (MODULE, "use m [f (g)]"),
            (
                "module m { export module n { export def f [] {} } }",
                "use m *",
            ),
            ("module m { export use n.nu f }", "use m *"),
            ("module m { export def f [] {} | ignore }", "use m *"),
            ("module m { export def m [] {} }", "use m *"),
            ("module m {\nexport\nexport def f [] {}\n}", "use m f"),
            ("module m { export def \"f\\tg\" [] {} }", "use m *"),
        ] {
            let src = format!("{module}\n{import}\nf");
            assert_eq!(callee(&src), Callee::Unknowable, "{src:?}");
        }
    }

    #[test]
    fn statement_exporting_no_command_leaves_module_readable() {
        let module = "module m {
            export const c = 1
            export-env { $env.A = 1 }
            def g [] {}
            export def f [] {}
        }";
        assert_eq!(callee(&format!("{module}\nuse m f\nf")), Callee::Defined);
    }

    #[test]
    fn module_import_of_alias_makes_command_unknowable() {
        for src in [
            "module m { export alias f = ls }\nuse m f\nf",
            "module m { export alias f = ls }\nuse m *\nf",
            "module m { export alias f = ls }\nuse m [f]\nf",
            "module m { export alias f = ls }\nuse m\nm f",
            "module m {\nexport alias f = ls\nexport def \"g h\" [] {}\n}\nuse m\nm f g",
            "def f [x: list] {}\nmodule m { export alias f = ls }\nuse m f\nf",
        ] {
            assert_eq!(callee(src), Callee::Unknowable, "{src:?}");
        }
    }

    #[test]
    fn module_import_of_alias_leaves_other_commands_defined() {
        let src = "module m {\nexport alias f = ls\nexport def g [] {}\n}\nuse m *\ng";
        assert_eq!(callee(src), Callee::Defined);
    }

    #[test]
    fn module_import_of_alias_leaves_earlier_call_defined() {
        let src = "module m { export alias f = ls }\ndef f [] {}\nf\nuse m f";
        assert_eq!(callee_on(src, 2), Callee::Defined);
    }

    #[test]
    fn outer_module_import_of_alias_makes_inner_command_unknowable() {
        assert_eq!(
            callee_nested(
                "module m { export alias f = ls }\nuse m f",
                "def g [] {}\nf"
            ),
            Callee::Unknowable
        );
    }

    #[test]
    fn module_import_of_member_takes_its_last_export() {
        let def = "export def \"f g\" [] {}";
        let alias = "export alias \"f g\" = ls";
        let src = |first, last| format!("module m {{\n{first}\n{last}\n}}\nuse m \"f g\"\nf g");
        assert_eq!(callee(&src(def, alias)), Callee::Unknowable);
        assert_eq!(callee(&src(alias, def)), Callee::Defined);
    }

    #[test]
    fn module_import_finds_exported_or_quoted_module() {
        for module in [
            "export module m { export def f [] {} }",
            "module \"m\" { export def f [] {} }",
        ] {
            let src = format!("{module}\nuse m f\nf");
            assert_eq!(callee(&src), Callee::Defined, "{src:?}");
        }
    }

    #[test]
    fn module_import_finds_no_later_module() {
        assert_eq!(
            callee("use m f\nmodule m { export def f [] {} }\nf"),
            Callee::Unknowable
        );
    }

    #[test]
    fn module_in_pipeline_declares_nothing() {
        assert_eq!(
            callee("module m { export def f [] {} } | ignore\nuse m f\nf"),
            Callee::Unknowable
        );
    }

    #[test]
    fn module_import_in_pipeline_makes_command_unknowable() {
        assert_eq!(
            callee(&format!("{MODULE}\nuse m f | ignore\nf")),
            Callee::Unknowable
        );
    }

    #[test]
    fn module_import_finds_last_earlier_module() {
        let src = "module m { export def f [] {} }\nmodule m { export def g [] {} }\nuse m *";
        assert_eq!(callee(&format!("{src}\ng")), Callee::Defined);
        assert_eq!(callee(&format!("{src}\nf")), Callee::Undefined);
    }

    #[test]
    fn inner_module_import_finds_outer_module() {
        assert_eq!(callee_nested(MODULE, "use m f\nf"), Callee::Defined);
    }

    #[test]
    fn inner_module_replaces_outer_module() {
        assert_eq!(
            callee_nested(MODULE, "module m { export def k [] {} }\nuse m *\nf"),
            Callee::Undefined
        );
    }

    #[test]
    fn module_import_finds_no_inner_module() {
        assert_eq!(
            callee("do { module m { export def f [] {} } }\nuse m f\nf"),
            Callee::Unknowable
        );
    }

    #[test]
    fn earlier_import_makes_module_command_unknowable() {
        for imports in ["use other.nu *\nuse m f", "use m f\nuse other.nu *"] {
            let src = format!("{MODULE}\n{imports}\nf");
            assert_eq!(callee(&src), Callee::Unknowable, "{src:?}");
        }
    }

    #[test]
    fn hidden_name_makes_module_command_unknowable() {
        assert_eq!(
            callee(&format!("{MODULE}\nuse m f\nhide f\nf")),
            Callee::Unknowable
        );
    }

    #[test]
    fn skipped_line_break_starts_no_import() {
        let src = "ls\nuse m.nu *\nf";
        assert_eq!(callee_lexed_in(src, 2, Mode::Paren), Callee::Undefined);
        assert_eq!(callee_lexed_in(src, 2, Mode::Block), Callee::Unknowable);
    }

    #[test]
    fn earlier_import_makes_longer_name_unknowable() {
        assert_eq!(callee("use m.nu *\ndef f [] {}\nf g"), Callee::Unknowable);
        assert_eq!(callee("def f [] {}\nf g"), Callee::Defined);
    }

    #[test]
    fn call_runs_longest_name_it_spells() {
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
    fn alias_heads_come_from_every_statement() {
        assert_eq!(*alias_heads("alias a = ^a"), ["a"]);
        assert_eq!(*alias_heads("export alias b = ^b"), ["b"]);
        assert_eq!(*alias_heads("print hi;alias c = ^c"), ["c"]);
        assert_eq!(*alias_heads("print hi\r\nalias d = ^d"), ["d"]);
        assert_eq!(*alias_heads("ls | alias e = ^e"), ["e"]);
        assert_eq!(*alias_heads("ls e>| alias e = ^e"), ["e"]);
        assert_eq!(*alias_heads("ls o+e>| alias e = ^e"), ["e"]);
        assert_eq!(*alias_heads("do { alias f = ^f }"), ["f"]);
        assert_eq!(*alias_heads("(alias g = ^g)"), ["g"]);
    }

    #[test]
    fn alias_heads_drop_quotes_around_name() {
        assert_eq!(*alias_heads("alias \"a\" = ^a"), ["a"]);
        assert_eq!(*alias_heads("alias 'b' = ^b"), ["b"]);
        assert_eq!(*alias_heads("alias `c` = ^c"), ["c"]);
    }

    #[test]
    fn alias_heads_come_sorted_without_repeats() {
        assert_eq!(
            *alias_heads("alias b = ^b\nalias a = ^a\nalias b = ^c"),
            ["a", "b"]
        );
    }

    #[test]
    fn alias_heads_are_first_words_of_names() {
        assert_eq!(
            *alias_heads("alias \"str x\" = ^a\nalias 'str y z' = ^b\nalias a = ^c"),
            ["a", "str"]
        );
    }

    #[test]
    fn alias_heads_omit_comments_and_strings() {
        assert!(alias_heads("# alias a = ^a").is_empty());
        assert!(alias_heads("print \"alias b = ^b\"").is_empty());
    }

    #[test]
    fn alias_heads_read_line_break_in_parentheses_as_space() {
        assert!(alias_heads("(ls\nalias a = ^a)").is_empty());
        assert_eq!(*alias_heads("(\nalias b = ^b\n)"), ["b"]);
        assert_eq!(*alias_heads("(do {\nls\nalias c = ^c\n})"), ["c"]);
        assert_eq!(*alias_heads("(# c\nalias d = ^d)"), ["d"]);
    }

    #[test]
    fn alias_heads_skip_brackets_and_quotes_in_strings_and_comments() {
        for (src, name) in [
            ("print \"\\\"(\"\nalias a = ^a", "a"),
            ("print '('\nalias b = ^b", "b"),
            ("print r#'it's ('#\nalias c = ^c", "c"),
            ("# (\nls\nalias d = ^d", "d"),
            ("ls;# it's\nalias e = ^e", "e"),
            ("ls |# it's\nalias f = ^f", "f"),
        ] {
            assert_eq!(*alias_heads(src), [name], "{src:?}");
        }
    }

    #[test]
    fn alias_heads_read_hash_inside_word_as_no_comment() {
        assert_eq!(*alias_heads("print [a#]\nalias a = ^a"), ["a"]);
    }

    #[test]
    fn alias_heads_omit_list_items() {
        assert!(alias_heads("[ls\nalias a = ^a]").is_empty());
        assert!(alias_heads("[alias b = ^b]").is_empty());
    }

    #[test]
    fn alias_heads_come_from_blocks_in_lists() {
        assert_eq!(*alias_heads("[{\nalias a = ^a\n}]"), ["a"]);
    }

    #[test]
    fn alias_heads_come_from_parentheses_in_interpolations() {
        assert_eq!(*alias_heads("print $\"(alias b = ^b; b)\""), ["b"]);
        assert_eq!(*alias_heads("print $'x((alias c = ^c))'"), ["c"]);
        assert!(alias_heads("print $\"(ls\nalias d = ^d)\"").is_empty());
        assert!(alias_heads("print $'(ls\nalias e = ^e)'").is_empty());
    }

    #[test]
    fn alias_heads_come_from_brackets_inside_words() {
        assert_eq!(*alias_heads("{a:{|| alias a = ^a; a}}"), ["a"]);
        assert_eq!(*alias_heads("ls --flag={ alias b = ^b }"), ["b"]);
        assert_eq!(*alias_heads("FOO=(alias c = ^c) ls"), ["c"]);
        assert_eq!(*alias_heads("ls --flag=(alias\nd = ^d)"), ["d"]);
        assert_eq!(*alias_heads("ls --flag={ export alias e = ^e }"), ["e"]);
        assert_eq!(
            *alias_heads("print {a:(do { alias f = ^f; f }), b: 1}"),
            ["f"]
        );
        assert_eq!(*alias_heads("ls --g=(do { alias g = ^g }),x"), ["g"]);
    }

    #[test]
    fn hidden_words_come_from_brackets_inside_words() {
        assert_eq!(
            hidden_words("{a:{|| hide m [d, e]}}").as_deref(),
            Some(&["d", "e", "m"][..])
        );
    }

    #[test]
    fn alias_heads_skip_comments_and_strings_in_brackets_inside_words() {
        assert!(alias_heads("{a:{|| # x alias a = ^a\na [b,  c]}}").is_empty());
        assert!(alias_heads("{a:{|| print \"x alias b = ^b\"; b}}").is_empty());
        assert!(alias_heads("print a\"{alias c = ^c}\"").is_empty());
    }

    fn nested(depth: usize, inner: &str) -> String {
        format!("{}{inner}{}", "(".repeat(depth), ")".repeat(depth))
    }

    #[test]
    fn alias_heads_come_from_brackets_as_deep_as_parser_reads() {
        assert_eq!(*alias_heads(&nested(MAX_DEPTH, "alias a = ^a")), ["a"]);
    }

    #[test]
    fn alias_heads_leave_out_alias_too_deep_to_read() {
        assert!(alias_heads(&nested(MAX_DEPTH + 1, "alias a = ^a")).is_empty());
    }

    #[test]
    fn alias_heads_lex_no_deeper_than_parser_reads() {
        // Lexing each of those brackets again would take minutes.
        let start = Instant::now();
        assert!(alias_heads(&nested(100_000, "alias a = ^a")).is_empty());
        let src = format!("alias b = ^b\n{}", nested(100_000, ""));
        assert_eq!(*alias_heads(&src), ["b"]);
        assert!(start.elapsed() < Duration::from_secs(30));
    }

    #[test]
    fn hide_too_deep_to_read_may_hide_every_word() {
        assert_eq!(hidden_words(&nested(MAX_DEPTH + 1, "hide a")), None);
    }

    #[test]
    fn alias_heads_omit_statement_naming_nothing() {
        assert!(alias_heads("alias").is_empty());
        assert!(alias_heads("export").is_empty());
        assert!(alias_heads("export alias").is_empty());
    }

    #[test]
    fn alias_heads_skip_alias_inside_other_word() {
        assert_eq!(*alias_heads("realias a = ^a\nalias b = ^b"), ["b"]);
    }

    #[test]
    fn hidden_words_come_from_every_statement() {
        assert_eq!(hidden_words("hide a").as_deref(), Some(&["a"][..]));
        assert_eq!(
            hidden_words("hide b\nhide c").as_deref(),
            Some(&["b", "c"][..])
        );
        assert_eq!(hidden_words("print hi;hide d").as_deref(), Some(&["d"][..]));
        assert_eq!(hidden_words("do { hide e }").as_deref(), Some(&["e"][..]));
        assert_eq!(hidden_words("hide f # g").as_deref(), Some(&["f"][..]));
    }

    #[test]
    fn hidden_words_read_line_break_in_parentheses_as_space() {
        assert_eq!(hidden_words("(ls\nhide\nf)").as_deref(), Some(&["f"][..]));
        assert_eq!(hidden_words("(ls hide f)").as_deref(), Some(&["f"][..]));
        assert_eq!(hidden_words("hide\nf").as_deref(), Some(&[][..]));
    }

    #[test]
    fn overlay_hide_in_any_statement_hides_every_word() {
        assert_eq!(hidden_words("hide a\noverlay hide o"), None);
    }
}
