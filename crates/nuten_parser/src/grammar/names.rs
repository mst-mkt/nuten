use std::{cmp::Reverse, iter, rc::Rc};

use nuten_syntax::SyntaxKind;
use rowan::{TextRange, TextSize};

use super::{
    builtin, is_pipe,
    parameters::{Declared, Item, Param},
};
use crate::lexer::{self, Mode, Token};

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
    /// A command a block around it defines, or one an import nuten reads brings.
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

    /// What the call with its head at `head` runs, as far as the blocks around it tell.
    /// `word` gives the text of each item of the call, `0` being the head.
    ///
    /// Nu runs the command of the longest name the items spell.
    /// It looks the name up in the innermost block that declares it.
    /// What a block imports takes the place of a name that block declares from there on.
    /// It takes the place of one an outer block declares as well.
    /// What an import nuten cannot read holds is unknowable, so it is taken to hold every name.
    /// Nu rejects a keyword as the name of a command, so no import brings one of those.
    /// An import of a module a block declares is read, and brings a command of each name it names.
    pub(crate) fn callee<'w>(
        &self,
        head: TextSize,
        word: impl Fn(usize) -> Option<&'w str>,
    ) -> Callee {
        self.resolved(head, word).0
    }

    /// What `callee` tells, and the definition the call runs if a block defines its command.
    fn resolved<'w>(
        &self,
        head: TextSize,
        word: impl Fn(usize) -> Option<&'w str>,
    ) -> (Callee, Option<(usize, &Scope<'a>, &Definition<'a>)>) {
        let spelled: Vec<_> = (0..self.longest_name()).map_while(&word).collect();
        let Some(defining @ (depth, scope, definition)) = self.defining(&spelled) else {
            let unknowable = self.scopes().any(|scope| scope.imports_before(head))
                && word(0).is_some_and(|head| !builtin::is_parser_keyword(head));
            let callee = if unknowable {
                Callee::Unknowable
            } else if !self
                .scopes()
                .any(|scope| scope.imports_spelled(&spelled, head))
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

    /// What nu takes each of `items` as, the items of the call with its head at `head`.
    /// `None` unless it runs a command a block defines, with a signature nuten reads.
    /// A command nu has may take its place where the name starts with a word that names one.
    /// Nu runs the one of the longer name then.
    /// It runs one an import nuten reads brings in place of the definition too.
    /// An import in the block of the definition does so only after it.
    /// Nu brings the definition in at its own statement, over any import before it.
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

    /// The scopes around the tokens, innermost first.
    fn scopes(&self) -> impl Iterator<Item = &Scope<'a>> {
        iter::successors(self.scope.as_deref(), |scope| scope.outer.as_deref())
    }

    /// How many words the longest name any block around the tokens defines or imports has.
    fn longest_name(&self) -> usize {
        self.scopes()
            .map(|scope| scope.longest_name)
            .max()
            .unwrap_or(0)
    }

    /// The command of the longest name `spelled` starts with among those the blocks define.
    /// With how many blocks out it stands, and the scope of that block.
    /// The innermost one where two blocks define a name that long.
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

/// A token of a block other than whitespace or a comment.
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

/// The words a block needs one of for its `Scope` to hold anything.
const SCOPE_WORDS: [&str; 6] = ["def", "extern", "module", "use", "source", "source-env"];

/// A block of statements, with what nu declares before it reads them and what they import.
struct Scope<'a> {
    /// Sorted by name, so that `longest_spelled` can bisect them.
    definitions: Box<[Definition<'a>]>,
    /// The commands the imports nuten reads bring, each starting where its import does.
    /// Sorted by name, then by where they start.
    imported: Box<[Definition<'a>]>,
    /// How many words the longest name among `definitions` and `imported` has.
    longest_name: usize,
    /// Where each statement that imports what nuten cannot read starts, in order.
    imports: Box<[TextSize]>,
    /// The modules the block declares, in order.
    modules: Box<[Module<'a>]>,
    outer: Option<Rc<Scope<'a>>>,
}

/// A command a `def` or an `extern` defines, as nu declares it before it reads the block.
/// Or one an import brings.
struct Definition<'a> {
    /// The words of the name, which a call spells out one item each.
    words: Vec<&'a str>,
    /// Where the statement starts; nu takes it over a name imported before there.
    start: TextSize,
    /// The parameters of a `def` nuten reads, which an import of it does not carry here.
    declared: Option<Declared<'a>>,
}

/// A module a `module` declares with a body, from where the statement starts.
struct Module<'a> {
    name: &'a str,
    start: TextSize,
    /// `None` where it exports what nuten cannot read.
    exports: Option<Exports<'a>>,
}

/// The commands a module exports.
struct Exports<'a> {
    /// The words of each name but `main`.
    commands: Vec<Vec<&'a str>>,
    /// Whether it exports `main`, which an import brings as the name of the module.
    main: bool,
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
                names.iter().map(|words| Definition {
                    words: words.clone(),
                    start: *start,
                    declared: None,
                })
            })
            .collect();
        imported.sort_by(|one, other| (&one.words, one.start).cmp(&(&other.words, other.start)));
        let imports: Box<[_]> = imports(lexemes)
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

    /// Whether an import nuten reads brings a command of a name `spelled` starts with before `end`.
    fn imports_spelled(&self, spelled: &[&str], end: TextSize) -> bool {
        (1..=spelled.len().min(self.longest_name)).any(|len| {
            let first = self
                .imported
                .partition_point(|command| command.words.as_slice() < &spelled[..len]);
            self.imported
                .get(first)
                .is_some_and(|command| command.words == spelled[..len] && command.start < end)
        })
    }

    /// Whether an import nuten reads brings a command of a name `spelled` starts with.
    /// The name has at least `len` words, and the import starts at or after `from`, before `end`.
    fn imports_named(&self, spelled: &[&str], len: usize, from: TextSize, end: TextSize) -> bool {
        self.imported.iter().any(|command| {
            command.words.len() >= len
                && spelled.starts_with(&command.words)
                && (from..end).contains(&command.start)
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

/// Where each import nuten reads among `statements` starts, with the names it brings.
/// `statements` are those of a block that stand alone, whose modules are `modules`.
fn read_imports<'a>(
    src: &'a str,
    statements: &[&[Lexeme<'a>]],
    modules: &[Module<'a>],
    outer: Option<&Scope<'a>>,
) -> Vec<(TextSize, Vec<Vec<&'a str>>)> {
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

/// The module named `name` that an import starting at `start` finds, if a block declares one.
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
    })
}

/// The parameters the signature `lexeme` declares, a `[ ]` or a `( )`.
/// The `:` before input and output types may touch it.
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

/// The statement, then what follows each assignment operator in it.
/// Nu reads each of those as a statement of the block.
fn assigned_parts<'l, 'a>(statement: &'l [Lexeme<'a>]) -> impl Iterator<Item = &'l [Lexeme<'a>]> {
    iter::once(statement).chain(
        statement
            .iter()
            .enumerate()
            .filter(|(_, lexeme)| lexeme.kind == SyntaxKind::Assign)
            .map(|(nth, _)| &statement[nth + 1..]),
    )
}

/// The module the statement declares, as `module` or `export module`, a name, and a body.
/// `None` for one with no body, which names a file nuten does not read.
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

/// What a module body exports, as its `export` statements tell.
/// `inside` is where the text inside its braces stands in `src`.
/// `None` where the body fails to lex, or where an `export` brings in what nuten cannot read.
/// An `export use` or an `export module` does, and so does a name nuten cannot tell.
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
            Export::Command(words) => Some(words),
            Export::Nothing | Export::Unreadable => None,
        })
        .collect::<Option<Vec<_>>>()?;
    // Nu takes an export named after its module for its `main`, which nuten does not read.
    if names.iter().any(|words| words.as_slice() == [module]) {
        return None;
    }
    let (main, commands): (Vec<_>, Vec<_>) = names
        .into_iter()
        .partition(|words| words.as_slice() == ["main"]);
    Some(Exports {
        commands,
        main: !main.is_empty(),
    })
}

#[derive(Debug)]
enum Export<'a> {
    Command(Vec<&'a str>),
    /// No command, as a `def` with no `export` or an `export const`.
    Nothing,
    Unreadable,
}

fn exported<'a>(src: &'a str, statements: &[&[Lexeme<'a>]], nth: usize) -> Export<'a> {
    let statement = statements[nth];
    let command = match statement {
        [export, keyword, ..] if export.text == "export" => match keyword.text {
            "def" | "extern" if stands_alone(statements, nth) => {
                definition(src, statement).map(|definition| definition.words)
            },
            "alias" if stands_alone(statements, nth) => statement
                .get(2)
                .and_then(|name| unquoted(name.text))
                .and_then(name_words),
            "const" => return Export::Nothing,
            _ => None,
        },
        [export] if export.text == "export" => None,
        _ => return Export::Nothing,
    };
    command.map_or(Export::Unreadable, Export::Command)
}

/// The names the import `statement` brings, as `use` or `export use`, a module, and any members.
/// `module` finds the module a name stands for, where a block declares it.
/// `None` where nuten cannot tell them all, as for a module it does not read.
/// An import with flags, or with more than one item of members, is left unread as well.
/// So is a member the module does not export, which nu rejects.
/// So is a member `main`, which nu reads apart from the other exports.
fn imported_names<'s, 'a: 's>(
    src: &'a str,
    statement: &[Lexeme<'a>],
    module: impl Fn(&str) -> Option<&'s Module<'a>>,
) -> Option<Vec<Vec<&'a str>>> {
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
    let main = exports.main.then(|| vec![module.name]);
    let Some(members) = members else {
        let prefixed = exports.commands.iter().map(|words| {
            iter::once(module.name)
                .chain(words.iter().copied())
                .collect()
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
    member_names(src, members)?
        .into_iter()
        .map(|member| {
            exports
                .commands
                .iter()
                .find(|words| words.iter().copied().eq(member.split(' ')))
                .cloned()
        })
        .collect()
}

/// The names of the members `members` spells: one name, or a list of them.
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

/// The tokens between the brackets `open` and `close` of `text`, lexed in `mode`.
/// Trivia and `,` are left out.
/// `text` stands at `start` in `src`.
/// `None` where the brackets do not enclose it, or where it fails to lex.
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
    let mut names: Vec<_> = statements_holding(src, "alias")
        .filter_map(alias_name)
        .collect();
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
    let statements = || statements_holding(src, "hide").map(str::split_ascii_whitespace);
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

/// The statements of `src` that hold `word`, each once, as `breaks_statement` splits them.
/// `word` holds no character that breaks a statement.
/// Most files hold no `alias` or `hide`, and finding the word is cheaper than splitting `src`.
fn statements_holding<'a>(src: &'a str, word: &'a str) -> impl Iterator<Item = &'a str> {
    let statement_from = move |from: usize| {
        let at = from + src[from..].find(word)?;
        let start = src[..at].rfind(breaks_statement).map_or(0, |end| end + 1);
        let end = src[at..]
            .find(breaks_statement)
            .map_or(src.len(), |end| at + end);
        Some(start..end)
    };
    iter::successors(statement_from(0), move |statement| {
        statement_from(statement.end)
    })
    .map(|statement| &src[statement])
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

    use super::{Callee, Names, alias_names, hidden_words};
    use crate::{
        grammar::parameters::Takes,
        lexer::{self, Mode},
    };

    /// What each word of the last line of `src` takes, as `defined_parameters` reads it.
    fn defined_takes(src: &str) -> Option<Vec<Option<Takes>>> {
        defined_takes_on(src, src.lines().count() - 1)
    }

    /// What each word of line `line` of `src` takes, as `defined_takes` tells of the last one.
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
        in_nested(outer, inner, |names, head, words| {
            names.callee(head, |nth| words.get(nth).copied())
        })
    }

    /// What each word of the last line of `inner` takes, with `inner` as a block inside `outer`.
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

    /// What `read` tells of the last line of `inner`, as a block inside that of `outer`.
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
    fn module_import_brings_exported_alias() {
        let src = "module m { export alias f = ls }\nuse m f\nf";
        assert_eq!(callee(src), Callee::Defined);
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
    fn skipped_line_break_makes_module_command_unknowable() {
        let src = format!("{MODULE}\nuse m f\nf");
        assert_eq!(callee_lexed_in(&src, 2, Mode::Paren), Callee::Unknowable);
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
    fn alias_names_omit_comments_and_strings() {
        assert!(alias_names("# alias a = ^a").is_empty());
        assert!(alias_names("print \"alias b = ^b\"").is_empty());
    }

    #[test]
    fn alias_names_omit_statement_naming_nothing() {
        assert!(alias_names("alias").is_empty());
        assert!(alias_names("export").is_empty());
        assert!(alias_names("export alias").is_empty());
    }

    #[test]
    fn alias_names_skip_alias_inside_other_word() {
        assert_eq!(*alias_names("realias a = ^a\nalias b = ^b"), ["b"]);
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
    }

    #[test]
    fn overlay_hide_in_any_statement_hides_every_word() {
        assert_eq!(hidden_words("hide a\noverlay hide o"), None);
    }
}
