//! Only a command with a parameter taking a `[ ]` as a list or an empty `{ }` as a closure is here.
//! Every other one takes a `[ ]` as text or rejects it, so it stays a word wherever it stands.
//! Nu reads an empty `{ }` there as a record wherever it stands.
//! A command nu reads a keyword syntax of is left out too, since its own rule reads it.
//! Where nuten does not read that syntax, what the command takes is unknown here.
//! None of these takes arguments it does not declare.

use std::iter;

use nuten_syntax::SyntaxKind;
use rowan::{TextRange, TextSize};

use super::{
    builtin::starts_with_parser_keyword,
    number::is_number,
    spread::{Target, is_spread},
};
use crate::lexer::{self, Mode};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Takes {
    List,
    ListOf,
    Word,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Param {
    pub(super) takes: Takes,
    negative: bool,
    /// A `oneof` reads `{}` by its first type, since reading a record reports no error there.
    pub(super) closure: bool,
}

const fn param(takes: Takes, negative: bool, closure: bool) -> Param {
    Param {
        takes,
        negative,
        closure,
    }
}

const LIST: Param = param(Takes::List, false, false);
const LIST_OF: Param = param(Takes::ListOf, false, false);
const WORD: Param = param(Takes::Word, false, false);
const NUMBER: Param = param(Takes::Word, true, false);
const CLOSURE: Param = param(Takes::Word, false, true);

#[derive(Debug)]
struct Flag<'s> {
    /// Empty for a flag with a short name alone.
    long: &'s str,
    short: Option<char>,
    value: Option<Param>,
}

const fn flag(long: &'static str, short: Option<char>, value: Option<Param>) -> Flag<'static> {
    Flag {
        long,
        short,
        value,
    }
}

#[derive(Debug)]
struct Signature<'s> {
    required: &'s [Param],
    optional: &'s [Param],
    rest: Option<Param>,
    flags: &'s [Flag<'s>],
}

impl Signature<'_> {
    fn positional(&self, index: usize) -> Option<Param> {
        self.required
            .iter()
            .chain(self.optional)
            .nth(index)
            .copied()
            .or(self.rest)
    }

    fn long(&self, name: &str) -> Option<&Flag<'_>> {
        self.flags
            .iter()
            .find(|flag| !flag.long.is_empty() && flag.long == name)
    }

    fn short(&self, name: char) -> Option<&Flag<'_>> {
        self.flags.iter().find(|flag| flag.short == Some(name))
    }
}

#[derive(Debug)]
struct Command {
    name: &'static [&'static str],
    /// `None` for one with no parameter here, which takes the place of a shorter name that has one.
    signature: Option<Signature<'static>>,
}

const fn command(name: &'static [&'static str], signature: Option<Signature<'static>>) -> Command {
    Command {
        name,
        signature,
    }
}

/// Every item after the head is a `WORD` when the command is none here, since its name is unknown.
/// A `WORD` reads a word of the name or a flag as it is, since neither is a `[ ]` nor a `{ }`.
pub(super) fn parameters(items: &[&str]) -> Vec<Option<Param>> {
    if spells_keyword(items) {
        return vec![None; items.len()];
    }
    let (words, signature) = match longest_spelled(items) {
        Some(command) => (command.name.len(), command.signature.as_ref()),
        None => (1, None),
    };
    let arguments = signature.map_or_else(
        || vec![Some(WORD); items.len().saturating_sub(words)],
        |signature| arguments(signature, &items[words..]),
    );
    iter::repeat_n(None, words.min(items.len()))
        .chain(arguments)
        .collect()
}

fn spells_keyword(items: &[&str]) -> bool {
    items
        .split_first()
        .is_some_and(|(head, rest)| starts_with_parser_keyword(head, rest.first().copied()))
}

fn longest_spelled(items: &[&str]) -> Option<&'static Command> {
    (1..=items.len()).rev().find_map(|len| {
        COMMANDS
            .binary_search_by(|command| command.name.cmp(&items[..len]))
            .ok()
            .map(|nth| &COMMANDS[nth])
    })
}

/// Ported from the loop of nu's `parse_internal_call` that goes through the arguments.
fn arguments(signature: &Signature<'_>, items: &[&str]) -> Vec<Option<Param>> {
    let mut taken = vec![None; items.len()];
    let mut positional = 0;
    let mut end_of_options = false;
    let mut at = 0;
    while let Some(&item) = items.get(at) {
        if !end_of_options && item == "--" {
            end_of_options = true;
        } else if !end_of_options && let Some(long) = item.strip_prefix("--") {
            // A value after `=` is part of the item, which stays as it is.
            let (name, joined) = match long.split_once('=') {
                Some((name, _)) => (name, true),
                None => (long, false),
            };
            if let Some(value) = signature.long(name).and_then(|flag| flag.value)
                && !joined
                && at + 1 < items.len()
            {
                at += 1;
                taken[at] = Some(value);
            }
        } else if !end_of_options && let Some(flags) = short_flags(signature, item, positional) {
            if let Some(value) = flags.last().and_then(|flag| flag.value)
                && at + 1 < items.len()
            {
                at += 1;
                taken[at] = Some(value);
            }
        } else if is_spread(item, Target::List) || is_spread(item, Target::Record) {
            // Nu takes a `{ }` spread as flags, and a `$` or `( )` one where the command has flags.
            // Every command has `--help`, which nu leaves out of that check.
            let has_flags = signature.flags.iter().any(|flag| flag.long != "help");
            let spreads_list =
                is_spread(item, Target::List) && (!is_spread(item, Target::Record) || !has_flags);
            if spreads_list && signature.rest.is_some() && positional >= signature.required.len() {
                positional = signature.required.len() + signature.optional.len();
            }
        } else {
            taken[at] = signature.positional(positional);
            positional += 1;
        }
        at += 1;
    }
    taken
}

/// Reads `item` as nu's `parse_short_flags` does.
/// Only the last flag may take a value; nu stops at one before it that does.
fn short_flags<'s>(
    signature: &'s Signature<'s>,
    item: &str,
    positional: usize,
) -> Option<Vec<&'s Flag<'s>>> {
    let names = item.strip_prefix('-').filter(|_| item.len() > 1)?;
    // Nu compares the byte offset of each flag with the number of chars.
    let count = names.chars().count();
    let flags: Vec<_> = names
        .char_indices()
        .filter_map(|(offset, name)| signature.short(name).map(|flag| (offset, flag)))
        .take_while(|&(offset, flag)| flag.value.is_none() || offset + 1 >= count)
        .map(|(_, flag)| flag)
        .collect();
    let negative_number = flags.is_empty()
        && signature
            .positional(positional)
            .is_some_and(|param| param.negative)
        && item.parse::<f64>().is_ok();
    (!negative_number).then_some(flags)
}

#[derive(Debug)]
pub(super) struct Declared<'a> {
    positional: Vec<Param>,
    required: usize,
    rest: Option<Param>,
    flags: Vec<Flag<'a>>,
}

impl<'a> Declared<'a> {
    fn signature(&self) -> Signature<'_> {
        let (required, optional) = self.positional.split_at(self.required);
        Signature {
            required,
            optional,
            rest: self.rest,
            flags: &self.flags,
        }
    }

    pub(super) fn arguments(&self, items: &[&str]) -> Vec<Option<Param>> {
        arguments(&self.signature(), items)
    }

    /// `items` are split as nu's `parse_signature_helper` splits them.
    pub(super) fn of(items: impl IntoIterator<Item = Item<'a>>) -> Option<Declared<'a>> {
        let drafts = drafts(items)?;
        let mut positional = Vec::new();
        let mut required = 0;
        let mut rest = None;
        let mut flags = Vec::new();
        for draft in drafts {
            let typed = match draft.ty {
                Some(ty) => Some(param_of_type(ty)?),
                None => None,
            };
            match draft.name {
                Name::Positional {
                    optional,
                } => {
                    let optional = optional || draft.default.is_some();
                    if !optional && positional.len() > required || rest.is_some() {
                        return None;
                    }
                    required += usize::from(!optional);
                    let param = match (typed, draft.default) {
                        (Some(typed), _) => typed,
                        (None, Some(default)) => param_of_default(default)?,
                        (None, None) => LIST,
                    };
                    positional.push(param);
                },
                Name::Rest if rest.is_none() && draft.default.is_none() => {
                    rest = Some(typed.unwrap_or(LIST));
                },
                Name::Rest => return None,
                Name::Flag {
                    long,
                    short,
                } => {
                    // Nu rejects `bool` as the type of a flag, which takes no value as a switch.
                    if draft.ty == Some("bool") {
                        return None;
                    }
                    let value = typed.or(draft.default.map(|_| WORD));
                    flags.push(Flag {
                        long,
                        short,
                        value,
                    });
                },
            }
        }
        Some(Declared {
            positional,
            required,
            rest,
            flags,
        })
    }
}

#[derive(Debug, Clone, Copy)]
pub(super) enum Item<'a> {
    Word(&'a str),
    Value,
    Colon,
    Eq,
    Other,
}

struct Draft<'a> {
    name: Name<'a>,
    ty: Option<&'a str>,
    default: Option<Item<'a>>,
}

enum Name<'a> {
    Positional { optional: bool },
    Rest,
    Flag { long: &'a str, short: Option<char> },
}

fn drafts<'a>(items: impl IntoIterator<Item = Item<'a>>) -> Option<Vec<Draft<'a>>> {
    let mut drafts: Vec<Draft<'a>> = Vec::new();
    let mut items = items.into_iter();
    while let Some(item) = items.next() {
        match (item, drafts.last_mut()) {
            (Item::Word(text), Some(draft)) if text.starts_with('(') => {
                let short = short_name(text.strip_prefix('(')?.strip_suffix(')')?)?;
                match &mut draft.name {
                    Name::Flag {
                        short: slot @ None,
                        ..
                    } if draft.ty.is_none() && draft.default.is_none() => *slot = Some(short),
                    _ => return None,
                }
            },
            (Item::Word(text), _) => drafts.push(Draft {
                name: name(text)?,
                ty: None,
                default: None,
            }),
            (Item::Colon, Some(draft)) if draft.ty.is_none() && draft.default.is_none() => {
                let Some(Item::Word(ty)) = items.next() else {
                    return None;
                };
                draft.ty = Some(ty);
            },
            (Item::Eq, Some(draft)) if draft.default.is_none() => {
                let default @ Some(Item::Word(_) | Item::Value) = items.next() else {
                    return None;
                };
                draft.default = default;
            },
            _ => return None,
        }
    }
    Some(drafts)
}

fn name(text: &str) -> Option<Name<'_>> {
    if text.starts_with(['(', '[', '{']) {
        return None;
    }
    if let Some(long) = text.strip_prefix("--") {
        let (long, short) = match long.split_once('(') {
            Some((long, short)) => (long, Some(short_name(short.strip_suffix(')')?)?)),
            None => (long, None),
        };
        return (!long.is_empty()).then_some(Name::Flag {
            long,
            short,
        });
    }
    if text.starts_with('-') {
        return Some(Name::Flag {
            long: "",
            short: Some(short_name(text)?),
        });
    }
    if let Some(rest) = text.strip_prefix("...") {
        return (!rest.is_empty()).then_some(Name::Rest);
    }
    Some(Name::Positional {
        optional: text.ends_with('?'),
    })
}

/// Nu gives a positional parameter with no type the type of its default value.
fn param_of_default(default: Item<'_>) -> Option<Param> {
    match default {
        Item::Word(text) if is_number(text) => Some(NUMBER),
        Item::Word(text) if text.starts_with(['[', '{', '(', '$']) || text == "null" => None,
        Item::Word(_) | Item::Value => Some(WORD),
        Item::Colon | Item::Eq | Item::Other => None,
    }
}

fn short_name(text: &str) -> Option<char> {
    let mut chars = text.strip_prefix('-')?.chars();
    match (chars.next(), chars.next()) {
        (Some(short), None) => Some(short),
        _ => None,
    }
}

/// Names types as nu's `parse_shape_name` does.
/// A custom completer after `@` is no part of the type.
fn param_of_type(text: &str) -> Option<Param> {
    let text = text.split('@').next()?;
    let names = match text
        .strip_prefix("oneof<")
        .and_then(|rest| rest.strip_suffix('>'))
    {
        Some(members) => without_commas(&type_param_tokens(members)),
        None => vec![text],
    };
    let members = names
        .iter()
        .copied()
        .map(reads)
        .collect::<Option<Vec<_>>>()
        .filter(|members| !members.is_empty())?;
    // Nu takes the first type that reads the `[ ]` with no error.
    // A list with an error nuten cannot see, as a variable that is not defined, falls to text.
    // So a `[ ]` stays a word where a type that takes text comes later.
    let accepting = members
        .iter()
        .position(|&member| member != Reads::Nothing && member != Reads::Number);
    let falls_to_text = accepting.is_some_and(|at| members[at + 1..].contains(&Reads::Text));
    let takes = match accepting.map(|at| members[at]) {
        Some(Reads::List) if !falls_to_text => Takes::List,
        Some(Reads::ListOf) if !falls_to_text => Takes::ListOf,
        _ => Takes::Word,
    };
    let closure = names.first().is_some_and(|name| name.trim() == "closure");
    Some(param(takes, members.contains(&Reads::Number), closure))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Reads {
    List,
    ListOf,
    Text,
    Number,
    Nothing,
}

fn reads(name: &str) -> Option<Reads> {
    let name: String = name.chars().filter(|char| !char.is_whitespace()).collect();
    Some(match name.as_str() {
        "any" | "list" | "list<any>" | "table" => Reads::List,
        name if name.starts_with("table<") => Reads::List,
        name if name.starts_with("list<") => Reads::ListOf,
        "string" | "path" | "glob" | "external_arg" => Reads::Text,
        "int" | "float" | "number" => Reads::Number,
        "binary" | "bool" | "cell-path" | "closure" | "datetime" | "directory" | "duration"
        | "error" | "filesize" | "nothing" | "range" | "record" => Reads::Nothing,
        name if name.starts_with("record<") => Reads::Nothing,
        _ => return None,
    })
}

/// Lexes the inside of a `< >` as nu's `parse_type_params` and `parse_named_type_params` do.
fn type_param_tokens(text: &str) -> Vec<(SyntaxKind, &str)> {
    lexer::tokens(text, TextRange::up_to(TextSize::of(text)), Mode::TypeParams)
        .filter(|token| !token.kind.is_trivia())
        .map(|token| (token.kind, &text[token.range]))
        .collect()
}

fn without_commas<'a>(tokens: &[(SyntaxKind, &'a str)]) -> Vec<&'a str> {
    tokens
        .iter()
        .filter(|&&(kind, _)| kind != SyntaxKind::Comma)
        .map(|&(_, text)| text)
        .collect()
}

/// Sorted by name, so that `longest_spelled` can bisect.
const COMMANDS: &[Command] = &[
    command(
        &["ansi"],
        Some(Signature {
            required: &[],
            optional: &[LIST],
            rest: None,
            flags: &[
                flag("escape", Some('e'), None),
                flag("osc", Some('o'), None),
                flag("list", Some('l'), None),
            ],
        }),
    ),
    command(&["ansi", "gradient"], None),
    command(&["ansi", "link"], None),
    command(&["ansi", "strip"], None),
    command(
        &["append"],
        Some(Signature {
            required: &[LIST],
            optional: &[],
            rest: Some(LIST),
            flags: &[],
        }),
    ),
    command(
        &["attr", "example"],
        Some(Signature {
            required: &[WORD, CLOSURE],
            optional: &[],
            rest: None,
            flags: &[flag("result", None, Some(LIST))],
        }),
    ),
    command(
        &["bytes", "build"],
        Some(Signature {
            required: &[],
            optional: &[],
            rest: Some(LIST),
            flags: &[],
        }),
    ),
    command(
        &["char"],
        Some(Signature {
            required: &[],
            optional: &[LIST],
            rest: Some(LIST),
            flags: &[
                flag("list", Some('l'), None),
                flag("unicode", Some('u'), None),
                flag("integer", Some('i'), None),
            ],
        }),
    ),
    command(
        &["collect"],
        Some(Signature {
            required: &[],
            optional: &[CLOSURE],
            rest: None,
            flags: &[flag("keep-env", None, None)],
        }),
    ),
    command(
        &["compact"],
        Some(Signature {
            required: &[],
            optional: &[],
            rest: Some(LIST),
            flags: &[flag("empty", Some('e'), None)],
        }),
    ),
    command(
        &["cp"],
        Some(Signature {
            required: &[],
            optional: &[],
            rest: Some(WORD),
            flags: &[
                flag("recursive", Some('r'), None),
                flag("no-dereference", Some('P'), None),
                flag("verbose", Some('v'), None),
                flag("force", Some('f'), None),
                flag("interactive", Some('i'), None),
                flag("update", Some('u'), None),
                flag("progress", Some('p'), None),
                flag("no-clobber", Some('n'), None),
                flag("preserve", None, Some(LIST_OF)),
                flag("debug", None, None),
                flag("all", Some('a'), None),
            ],
        }),
    ),
    command(
        &["debug", "profile"],
        Some(Signature {
            required: &[CLOSURE],
            optional: &[],
            rest: None,
            flags: &[
                flag("spans", Some('s'), None),
                flag("expand-source", Some('e'), None),
                flag("values", Some('v'), None),
                flag("lines", Some('l'), None),
                flag("duration-values", Some('d'), None),
                flag("max-depth", Some('m'), Some(NUMBER)),
            ],
        }),
    ),
    command(
        &["default"],
        Some(Signature {
            required: &[LIST],
            optional: &[],
            rest: Some(WORD),
            flags: &[flag("empty", Some('e'), None)],
        }),
    ),
    command(
        &["difference"],
        Some(Signature {
            required: &[LIST],
            optional: &[],
            rest: None,
            flags: &[],
        }),
    ),
    command(
        &["do"],
        Some(Signature {
            required: &[CLOSURE],
            optional: &[],
            rest: Some(LIST),
            flags: &[
                flag("ignore-errors", Some('i'), None),
                flag("capture-errors", Some('c'), None),
                flag("env", None, None),
            ],
        }),
    ),
    command(
        &["drop", "nth"],
        Some(Signature {
            required: &[],
            optional: &[],
            rest: Some(LIST),
            flags: &[],
        }),
    ),
    command(
        &["each"],
        Some(Signature {
            required: &[CLOSURE],
            optional: &[],
            rest: None,
            flags: &[
                flag("keep-empty", Some('k'), None),
                flag("flatten", Some('f'), None),
            ],
        }),
    ),
    command(
        &["each", "while"],
        Some(Signature {
            required: &[CLOSURE],
            optional: &[],
            rest: None,
            flags: &[],
        }),
    ),
    command(
        &["echo"],
        Some(Signature {
            required: &[],
            optional: &[],
            rest: Some(LIST),
            flags: &[],
        }),
    ),
    command(
        &["explain"],
        Some(Signature {
            required: &[CLOSURE],
            optional: &[],
            rest: None,
            flags: &[],
        }),
    ),
    command(
        &["filter"],
        Some(Signature {
            required: &[CLOSURE],
            optional: &[],
            rest: None,
            flags: &[],
        }),
    ),
    command(
        &["find"],
        Some(Signature {
            required: &[],
            optional: &[],
            rest: Some(LIST),
            flags: &[
                flag("regex", Some('r'), Some(WORD)),
                flag("ignore-case", Some('i'), None),
                flag("multiline", Some('m'), None),
                flag("dotall", Some('s'), None),
                flag("columns", Some('c'), Some(LIST_OF)),
                flag("no-highlight", Some('n'), None),
                flag("invert", Some('v'), None),
                flag("rfind", Some('R'), None),
            ],
        }),
    ),
    command(
        &["from", "ods"],
        Some(Signature {
            required: &[],
            optional: &[],
            rest: None,
            flags: &[
                flag("sheets", Some('s'), Some(LIST_OF)),
                flag("noheaders", Some('n'), None),
                flag("first-row", Some('f'), Some(NUMBER)),
                flag("prefer-integers", Some('i'), None),
            ],
        }),
    ),
    command(
        &["from", "xlsx"],
        Some(Signature {
            required: &[],
            optional: &[],
            rest: None,
            flags: &[
                flag("sheets", Some('s'), Some(LIST_OF)),
                flag("noheaders", Some('n'), None),
                flag("first-row", Some('f'), Some(NUMBER)),
                flag("prefer-integers", Some('i'), None),
            ],
        }),
    ),
    command(
        &["generate"],
        Some(Signature {
            required: &[CLOSURE],
            optional: &[LIST],
            rest: None,
            flags: &[],
        }),
    ),
    command(
        &["glob"],
        Some(Signature {
            required: &[WORD],
            optional: &[],
            rest: None,
            flags: &[
                flag("depth", Some('d'), Some(NUMBER)),
                flag("no-dir", Some('D'), None),
                flag("no-file", Some('F'), None),
                flag("no-symlink", Some('S'), None),
                flag("follow-symlinks", Some('l'), None),
                flag("exclude", Some('e'), Some(LIST_OF)),
            ],
        }),
    ),
    command(
        &["http"],
        Some(Signature {
            required: &[],
            optional: &[WORD, LIST],
            rest: None,
            flags: &[
                flag("content-type", Some('t'), Some(LIST)),
                flag("user", Some('u'), Some(LIST)),
                flag("password", Some('p'), Some(LIST)),
                flag("max-time", Some('m'), Some(WORD)),
                flag("headers", Some('H'), Some(LIST)),
                flag("raw", Some('r'), None),
                flag("insecure", Some('k'), None),
                flag("full", Some('f'), None),
                flag("allow-errors", Some('e'), None),
                flag("redirect-mode", Some('R'), Some(WORD)),
            ],
        }),
    ),
    command(
        &["http", "delete"],
        Some(Signature {
            required: &[WORD],
            optional: &[],
            rest: None,
            flags: &[
                flag("user", Some('u'), Some(LIST)),
                flag("password", Some('p'), Some(LIST)),
                flag("data", Some('d'), Some(LIST)),
                flag("content-type", Some('t'), Some(LIST)),
                flag("max-time", Some('m'), Some(WORD)),
                flag("headers", Some('H'), Some(LIST)),
                flag("raw", Some('r'), None),
                flag("insecure", Some('k'), None),
                flag("full", Some('f'), None),
                flag("allow-errors", Some('e'), None),
                flag("pool", None, None),
                flag("redirect-mode", Some('R'), Some(WORD)),
                flag("unix-socket", Some('U'), Some(WORD)),
            ],
        }),
    ),
    command(
        &["http", "get"],
        Some(Signature {
            required: &[WORD],
            optional: &[],
            rest: None,
            flags: &[
                flag("user", Some('u'), Some(LIST)),
                flag("password", Some('p'), Some(LIST)),
                flag("max-time", Some('m'), Some(WORD)),
                flag("headers", Some('H'), Some(LIST)),
                flag("raw", Some('r'), None),
                flag("insecure", Some('k'), None),
                flag("full", Some('f'), None),
                flag("allow-errors", Some('e'), None),
                flag("pool", None, None),
                flag("redirect-mode", Some('R'), Some(WORD)),
                flag("unix-socket", Some('U'), Some(WORD)),
            ],
        }),
    ),
    command(
        &["http", "head"],
        Some(Signature {
            required: &[WORD],
            optional: &[],
            rest: None,
            flags: &[
                flag("user", Some('u'), Some(LIST)),
                flag("password", Some('p'), Some(LIST)),
                flag("max-time", Some('m'), Some(WORD)),
                flag("headers", Some('H'), Some(LIST)),
                flag("full", Some('f'), None),
                flag("insecure", Some('k'), None),
                flag("allow-errors", Some('e'), None),
                flag("pool", None, None),
                flag("redirect-mode", Some('R'), Some(WORD)),
                flag("unix-socket", Some('U'), Some(WORD)),
            ],
        }),
    ),
    command(
        &["http", "options"],
        Some(Signature {
            required: &[WORD],
            optional: &[],
            rest: None,
            flags: &[
                flag("user", Some('u'), Some(LIST)),
                flag("password", Some('p'), Some(LIST)),
                flag("max-time", Some('m'), Some(WORD)),
                flag("headers", Some('H'), Some(LIST)),
                flag("full", Some('f'), None),
                flag("insecure", Some('k'), None),
                flag("allow-errors", Some('e'), None),
                flag("pool", None, None),
                flag("unix-socket", Some('U'), Some(WORD)),
            ],
        }),
    ),
    command(
        &["http", "patch"],
        Some(Signature {
            required: &[WORD],
            optional: &[LIST],
            rest: None,
            flags: &[
                flag("user", Some('u'), Some(LIST)),
                flag("password", Some('p'), Some(LIST)),
                flag("content-type", Some('t'), Some(LIST)),
                flag("max-time", Some('m'), Some(WORD)),
                flag("headers", Some('H'), Some(LIST)),
                flag("raw", Some('r'), None),
                flag("insecure", Some('k'), None),
                flag("full", Some('f'), None),
                flag("allow-errors", Some('e'), None),
                flag("pool", None, None),
                flag("redirect-mode", Some('R'), Some(WORD)),
                flag("unix-socket", Some('U'), Some(WORD)),
            ],
        }),
    ),
    command(&["http", "pool"], None),
    command(
        &["http", "post"],
        Some(Signature {
            required: &[WORD],
            optional: &[LIST],
            rest: None,
            flags: &[
                flag("user", Some('u'), Some(LIST)),
                flag("password", Some('p'), Some(LIST)),
                flag("content-type", Some('t'), Some(LIST)),
                flag("max-time", Some('m'), Some(WORD)),
                flag("headers", Some('H'), Some(LIST)),
                flag("raw", Some('r'), None),
                flag("insecure", Some('k'), None),
                flag("full", Some('f'), None),
                flag("allow-errors", Some('e'), None),
                flag("pool", None, None),
                flag("redirect-mode", Some('R'), Some(WORD)),
                flag("unix-socket", Some('U'), Some(WORD)),
            ],
        }),
    ),
    command(
        &["http", "put"],
        Some(Signature {
            required: &[WORD],
            optional: &[LIST],
            rest: None,
            flags: &[
                flag("user", Some('u'), Some(LIST)),
                flag("password", Some('p'), Some(LIST)),
                flag("content-type", Some('t'), Some(LIST)),
                flag("max-time", Some('m'), Some(WORD)),
                flag("headers", Some('H'), Some(LIST)),
                flag("raw", Some('r'), None),
                flag("insecure", Some('k'), None),
                flag("full", Some('f'), None),
                flag("allow-errors", Some('e'), None),
                flag("pool", None, None),
                flag("redirect-mode", Some('R'), Some(WORD)),
                flag("unix-socket", Some('U'), Some(WORD)),
            ],
        }),
    ),
    command(
        &["idx", "watch"],
        Some(Signature {
            required: &[],
            optional: &[WORD],
            rest: None,
            flags: &[
                flag("ignore", Some('i'), Some(LIST_OF)),
                flag("timeout", Some('t'), Some(WORD)),
                flag("max-events", Some('n'), Some(NUMBER)),
            ],
        }),
    ),
    command(
        &["input", "listen"],
        Some(Signature {
            required: &[],
            optional: &[],
            rest: None,
            flags: &[
                flag("types", Some('t'), Some(LIST_OF)),
                flag("raw", Some('r'), None),
                flag("timeout", Some('o'), Some(WORD)),
            ],
        }),
    ),
    command(
        &["insert"],
        Some(Signature {
            required: &[WORD, LIST],
            optional: &[],
            rest: None,
            flags: &[],
        }),
    ),
    command(
        &["interleave"],
        Some(Signature {
            required: &[],
            optional: &[],
            rest: Some(CLOSURE),
            flags: &[flag("buffer-size", Some('b'), Some(NUMBER))],
        }),
    ),
    command(
        &["intersect"],
        Some(Signature {
            required: &[LIST],
            optional: &[],
            rest: None,
            flags: &[],
        }),
    ),
    command(
        &["items"],
        Some(Signature {
            required: &[CLOSURE],
            optional: &[],
            rest: None,
            flags: &[],
        }),
    ),
    command(
        &["job", "spawn"],
        Some(Signature {
            required: &[CLOSURE],
            optional: &[],
            rest: None,
            flags: &[flag("description", Some('d'), Some(WORD))],
        }),
    ),
    command(
        &["join"],
        Some(Signature {
            required: &[LIST, WORD],
            optional: &[WORD],
            rest: None,
            flags: &[
                flag("prefix", Some('p'), Some(WORD)),
                flag("suffix", Some('s'), Some(WORD)),
                flag("inner", Some('i'), None),
                flag("left", Some('l'), None),
                flag("right", Some('r'), None),
                flag("outer", Some('o'), None),
            ],
        }),
    ),
    command(
        &["matrix", "add"],
        Some(Signature {
            required: &[LIST],
            optional: &[],
            rest: None,
            flags: &[flag("broadcast", Some('b'), None)],
        }),
    ),
    command(
        &["matrix", "map"],
        Some(Signature {
            required: &[CLOSURE],
            optional: &[],
            rest: None,
            flags: &[],
        }),
    ),
    command(
        &["matrix", "multiply"],
        Some(Signature {
            required: &[LIST],
            optional: &[],
            rest: Some(LIST),
            flags: &[
                flag("swap", Some('s'), None),
                flag("multall", Some('a'), None),
            ],
        }),
    ),
    command(
        &["matrix", "reduce"],
        Some(Signature {
            required: &[CLOSURE],
            optional: &[],
            rest: None,
            flags: &[flag("fold", Some('f'), Some(LIST))],
        }),
    ),
    command(
        &["matrix", "set-col"],
        Some(Signature {
            required: &[NUMBER, LIST_OF],
            optional: &[],
            rest: None,
            flags: &[],
        }),
    ),
    command(
        &["matrix", "set-row"],
        Some(Signature {
            required: &[NUMBER, LIST_OF],
            optional: &[],
            rest: None,
            flags: &[],
        }),
    ),
    command(
        &["matrix", "subtract"],
        Some(Signature {
            required: &[LIST],
            optional: &[],
            rest: None,
            flags: &[flag("broadcast", Some('b'), None)],
        }),
    ),
    command(
        &["merge"],
        Some(Signature {
            required: &[LIST],
            optional: &[],
            rest: None,
            flags: &[],
        }),
    ),
    command(
        &["merge", "deep"],
        Some(Signature {
            required: &[LIST],
            optional: &[],
            rest: None,
            flags: &[flag("strategy", Some('s'), Some(WORD))],
        }),
    ),
    command(
        &["metadata"],
        Some(Signature {
            required: &[],
            optional: &[LIST],
            rest: None,
            flags: &[],
        }),
    ),
    command(
        &["metadata", "access"],
        Some(Signature {
            required: &[CLOSURE],
            optional: &[],
            rest: None,
            flags: &[],
        }),
    ),
    command(
        &["metadata", "set"],
        Some(Signature {
            required: &[],
            optional: &[CLOSURE],
            rest: None,
            flags: &[
                flag("datasource-filepath", Some('f'), Some(WORD)),
                flag("path-columns", Some('p'), Some(LIST_OF)),
                flag("content-type", Some('c'), Some(WORD)),
                flag("table-width-priority-columns", Some('w'), Some(LIST_OF)),
            ],
        }),
    ),
    command(
        &["par-each"],
        Some(Signature {
            required: &[CLOSURE],
            optional: &[],
            rest: None,
            flags: &[
                flag("threads", Some('t'), Some(NUMBER)),
                flag("keep-order", Some('k'), None),
            ],
        }),
    ),
    command(
        &["prepend"],
        Some(Signature {
            required: &[LIST],
            optional: &[],
            rest: None,
            flags: &[],
        }),
    ),
    command(
        &["print"],
        Some(Signature {
            required: &[],
            optional: &[],
            rest: Some(LIST),
            flags: &[
                flag("no-newline", Some('n'), None),
                flag("stderr", Some('e'), None),
                flag("raw", Some('r'), None),
            ],
        }),
    ),
    command(
        &["query", "db"],
        Some(Signature {
            required: &[WORD],
            optional: &[],
            rest: None,
            flags: &[flag("params", Some('p'), Some(LIST))],
        }),
    ),
    command(
        &["reduce"],
        Some(Signature {
            required: &[CLOSURE],
            optional: &[],
            rest: None,
            flags: &[flag("fold", Some('f'), Some(LIST))],
        }),
    ),
    command(
        &["rename"],
        Some(Signature {
            required: &[],
            optional: &[],
            rest: Some(WORD),
            flags: &[
                flag("column", Some('c'), Some(WORD)),
                flag("block", Some('b'), Some(CLOSURE)),
            ],
        }),
    ),
    command(
        &["run-internal"],
        Some(Signature {
            required: &[WORD],
            optional: &[],
            rest: Some(LIST),
            flags: &[],
        }),
    ),
    command(
        &["split", "list"],
        Some(Signature {
            required: &[LIST],
            optional: &[],
            rest: None,
            flags: &[
                flag("regex", Some('r'), None),
                flag("split", None, Some(WORD)),
            ],
        }),
    ),
    command(
        &["str", "substring"],
        Some(Signature {
            required: &[LIST],
            optional: &[],
            rest: Some(WORD),
            flags: &[
                flag("grapheme-clusters", Some('g'), None),
                flag("utf-8-bytes", Some('b'), None),
            ],
        }),
    ),
    command(
        &["table"],
        Some(Signature {
            required: &[],
            optional: &[],
            rest: None,
            flags: &[
                flag("theme", Some('t'), Some(WORD)),
                flag("index", Some('i'), Some(LIST)),
                flag("width", Some('w'), Some(NUMBER)),
                flag("expand", Some('e'), None),
                flag("expand-deep", Some('d'), Some(NUMBER)),
                flag("flatten", None, None),
                flag("flatten-separator", None, Some(WORD)),
                flag("collapse", Some('c'), None),
                flag("abbreviated", Some('a'), Some(NUMBER)),
                flag("list", Some('l'), None),
                flag("icons", Some('o'), None),
            ],
        }),
    ),
    command(
        &["tee"],
        Some(Signature {
            required: &[CLOSURE],
            optional: &[],
            rest: None,
            flags: &[flag("stderr", Some('e'), None)],
        }),
    ),
    command(
        &["timeit"],
        Some(Signature {
            required: &[CLOSURE],
            optional: &[],
            rest: None,
            flags: &[flag("output", Some('o'), None)],
        }),
    ),
    command(
        &["to", "csv"],
        Some(Signature {
            required: &[],
            optional: &[],
            rest: None,
            flags: &[
                flag("separator", Some('s'), Some(WORD)),
                flag("noheaders", Some('n'), None),
                flag("columns", None, Some(LIST_OF)),
            ],
        }),
    ),
    command(
        &["to", "md"],
        Some(Signature {
            required: &[],
            optional: &[],
            rest: None,
            flags: &[
                flag("pretty", Some('p'), None),
                flag("per-element", Some('e'), None),
                flag("center", Some('c'), Some(LIST_OF)),
                flag("escape-md", Some('m'), None),
                flag("escape-html", Some('t'), None),
                flag("escape-all", Some('a'), None),
                flag("list", Some('l'), Some(WORD)),
            ],
        }),
    ),
    command(
        &["to", "tsv"],
        Some(Signature {
            required: &[],
            optional: &[],
            rest: None,
            flags: &[
                flag("noheaders", Some('n'), None),
                flag("columns", None, Some(LIST_OF)),
            ],
        }),
    ),
    command(
        &["tui", "bind"],
        Some(Signature {
            required: &[WORD, CLOSURE],
            optional: &[],
            rest: None,
            flags: &[],
        }),
    ),
    command(
        &["tui", "box"],
        Some(Signature {
            required: &[WORD, LIST],
            optional: &[],
            rest: None,
            flags: &[flag("id", None, Some(WORD)), flag("focus", None, None)],
        }),
    ),
    command(
        &["tui", "button"],
        Some(Signature {
            required: &[WORD],
            optional: &[CLOSURE],
            rest: None,
            flags: &[flag("id", None, Some(WORD)), flag("focus", None, None)],
        }),
    ),
    command(
        &["tui", "debug"],
        Some(Signature {
            required: &[],
            optional: &[CLOSURE],
            rest: None,
            flags: &[
                flag("keys", Some('k'), Some(WORD)),
                flag("until", Some('u'), Some(CLOSURE)),
                flag("size", Some('s'), Some(LIST_OF)),
                flag("dialog", Some('d'), None),
            ],
        }),
    ),
    command(
        &["tui", "label"],
        Some(Signature {
            required: &[],
            optional: &[WORD],
            rest: None,
            flags: &[
                flag("title", None, None),
                flag("status", None, None),
                flag("id", None, Some(WORD)),
                flag("focus", None, None),
                flag("data", None, Some(LIST)),
                flag("from", None, Some(WORD)),
            ],
        }),
    ),
    command(
        &["tui", "log"],
        Some(Signature {
            required: &[],
            optional: &[CLOSURE],
            rest: None,
            flags: &[
                flag("max-lines", None, Some(NUMBER)),
                flag("id", None, Some(WORD)),
                flag("focus", None, None),
                flag("data", None, Some(LIST)),
                flag("from", None, Some(WORD)),
            ],
        }),
    ),
    command(
        &["tui", "menu"],
        Some(Signature {
            required: &[LIST],
            optional: &[],
            rest: None,
            flags: &[flag("id", None, Some(WORD)), flag("focus", None, None)],
        }),
    ),
    command(
        &["tui", "preview"],
        Some(Signature {
            required: &[],
            optional: &[CLOSURE],
            rest: None,
            flags: &[
                flag("from", None, Some(WORD)),
                flag("max-bytes", None, Some(NUMBER)),
                flag("id", None, Some(WORD)),
                flag("focus", None, None),
            ],
        }),
    ),
    command(
        &["tui", "progress"],
        Some(Signature {
            required: &[],
            optional: &[CLOSURE],
            rest: None,
            flags: &[
                flag("value", Some('v'), Some(NUMBER)),
                flag("total", Some('t'), Some(NUMBER)),
                flag("label", Some('l'), Some(WORD)),
                flag("id", None, Some(WORD)),
                flag("focus", None, None),
                flag("data", None, Some(LIST)),
                flag("from", None, Some(WORD)),
            ],
        }),
    ),
    command(
        &["tui", "run"],
        Some(Signature {
            required: &[],
            optional: &[CLOSURE],
            rest: None,
            flags: &[
                flag("dialog", Some('d'), None),
                flag("size", Some('s'), Some(LIST_OF)),
                flag("refresh", Some('r'), Some(WORD)),
                flag("no-mouse", None, None),
            ],
        }),
    ),
    command(
        &["tui", "search"],
        Some(Signature {
            required: &[],
            optional: &[],
            rest: None,
            flags: &[
                flag("placeholder", None, Some(WORD)),
                flag("bind", None, Some(WORD)),
                flag("fuzzy", Some('f'), None),
                flag("case-sensitive", Some('s'), None),
                flag("columns", None, Some(LIST_OF)),
                flag("id", None, Some(WORD)),
                flag("focus", None, None),
            ],
        }),
    ),
    command(
        &["tui", "select"],
        Some(Signature {
            required: &[],
            optional: &[LIST],
            rest: None,
            flags: &[
                flag("display", Some('d'), Some(WORD)),
                flag("index", Some('i'), None),
                flag("id", None, Some(WORD)),
                flag("focus", None, None),
                flag("data", None, Some(LIST)),
                flag("from", None, Some(WORD)),
                flag("on-select", None, Some(CLOSURE)),
                flag("multi", Some('m'), None),
            ],
        }),
    ),
    command(
        &["tui", "split"],
        Some(Signature {
            required: &[LIST],
            optional: &[],
            rest: None,
            flags: &[
                flag("vertical", Some('v'), None),
                flag("sizes", Some('s'), Some(LIST)),
                flag("ratio", Some('r'), Some(NUMBER)),
                flag("id", None, Some(WORD)),
                flag("focus", None, None),
            ],
        }),
    ),
    command(
        &["tui", "tab"],
        Some(Signature {
            required: &[WORD, LIST],
            optional: &[],
            rest: None,
            flags: &[flag("id", None, Some(WORD)), flag("focus", None, None)],
        }),
    ),
    command(
        &["tui", "table"],
        Some(Signature {
            required: &[],
            optional: &[CLOSURE],
            rest: None,
            flags: &[
                flag("columns", None, Some(LIST_OF)),
                flag("capture-keys", None, None),
                flag("index", Some('i'), None),
                flag("id", None, Some(WORD)),
                flag("focus", None, None),
                flag("data", None, Some(LIST)),
                flag("from", None, Some(WORD)),
                flag("on-select", None, Some(CLOSURE)),
                flag("multi", Some('m'), None),
            ],
        }),
    ),
    command(
        &["tui", "tree"],
        Some(Signature {
            required: &[],
            optional: &[CLOSURE],
            rest: None,
            flags: &[
                flag("walk", None, None),
                flag("column", None, Some(WORD)),
                flag("id", None, Some(WORD)),
                flag("focus", None, None),
                flag("data", None, Some(LIST)),
                flag("from", None, Some(WORD)),
                flag("on-select", None, Some(CLOSURE)),
                flag("multi", Some('m'), None),
            ],
        }),
    ),
    command(
        &["ulimit"],
        Some(Signature {
            required: &[],
            optional: &[LIST],
            rest: None,
            flags: &[
                flag("soft", Some('S'), None),
                flag("hard", Some('H'), None),
                flag("all", Some('a'), None),
                flag("core-size", Some('c'), None),
                flag("data-size", Some('d'), None),
                flag("nice", Some('e'), None),
                flag("file-size", Some('f'), None),
                flag("pending-signals", Some('i'), None),
                flag("lock-size", Some('l'), None),
                flag("resident-set-size", Some('m'), None),
                flag("file-descriptor-count", Some('n'), None),
                flag("queue-size", Some('q'), None),
                flag("realtime-priority", Some('r'), None),
                flag("stack-size", Some('s'), None),
                flag("cpu-time", Some('t'), None),
                flag("process-count", Some('u'), None),
                flag("virtual-memory-size", Some('v'), None),
                flag("file-locks", Some('x'), None),
                flag("realtime-maxtime", Some('y'), None),
            ],
        }),
    ),
    command(
        &["union"],
        Some(Signature {
            required: &[LIST],
            optional: &[],
            rest: None,
            flags: &[],
        }),
    ),
    command(
        &["uniq-by"],
        Some(Signature {
            required: &[],
            optional: &[],
            rest: Some(LIST),
            flags: &[
                flag("count", Some('c'), None),
                flag("keep-last", Some('l'), None),
                flag("repeated", Some('d'), None),
                flag("ignore-case", Some('i'), None),
                flag("unique", Some('u'), None),
            ],
        }),
    ),
    command(
        &["unlet"],
        Some(Signature {
            required: &[],
            optional: &[],
            rest: Some(LIST),
            flags: &[],
        }),
    ),
    command(
        &["update"],
        Some(Signature {
            required: &[WORD, LIST],
            optional: &[],
            rest: None,
            flags: &[],
        }),
    ),
    command(
        &["update", "cells"],
        Some(Signature {
            required: &[CLOSURE],
            optional: &[],
            rest: None,
            flags: &[
                flag("columns", Some('c'), Some(LIST)),
                flag("recursive", Some('r'), None),
            ],
        }),
    ),
    command(
        &["upsert"],
        Some(Signature {
            required: &[WORD, LIST],
            optional: &[],
            rest: None,
            flags: &[],
        }),
    ),
    command(
        &["view", "ir"],
        Some(Signature {
            required: &[LIST],
            optional: &[],
            rest: None,
            flags: &[
                flag("json", Some('j'), None),
                flag("decl-id", Some('d'), None),
            ],
        }),
    ),
    command(
        &["view", "source"],
        Some(Signature {
            required: &[LIST],
            optional: &[],
            rest: None,
            flags: &[flag("dependencies", Some('d'), None)],
        }),
    ),
    command(
        &["watch"],
        Some(Signature {
            required: &[WORD],
            optional: &[CLOSURE],
            rest: None,
            flags: &[
                flag("debounce", Some('d'), Some(WORD)),
                flag("glob", Some('g'), Some(WORD)),
                flag("recursive", Some('r'), Some(WORD)),
                flag("quiet", Some('q'), None),
                flag("verbose", Some('v'), None),
            ],
        }),
    ),
    command(
        &["with-env"],
        Some(Signature {
            required: &[LIST, CLOSURE],
            optional: &[],
            rest: None,
            flags: &[],
        }),
    ),
    command(
        &["zip"],
        Some(Signature {
            required: &[LIST],
            optional: &[],
            rest: None,
            flags: &[],
        }),
    ),
];

#[cfg(test)]
mod tests {
    use super::{
        CLOSURE, COMMANDS, Command, Declared, Item, LIST, LIST_OF, NUMBER, Param, Takes, WORD,
        param_of_type, parameters,
    };
    use crate::test_support::asked;

    /// A keyword nu reads a syntax of is left out, as nu's refusal to `def` one tells.
    const COMMANDS_OF_NU: &str = r#"
        def split-top [text: string] {
            $text | split chars | reduce --fold {parts: [], depth: 0, cur: ''} {|ch, acc|
                let step = if $ch in ['<' '('] { 1 } else if $ch in ['>' ')'] { -1 } else { 0 }
                let depth = $acc.depth + $step
                if $ch == ',' and $depth == 0 {
                    let parts = $acc.parts | append ($acc.cur | str trim)
                    {parts: $parts, depth: $depth, cur: ''}
                } else {
                    {parts: $acc.parts, depth: $depth, cur: ($acc.cur + $ch)}
                }
            } | $in.parts ++ [($in.cur | str trim)]
        }
        def members [shape: string] {
            if ($shape | str starts-with 'oneof<') {
                split-top ($shape | str substring 6..-2)
            } else {
                [$shape]
            }
        }
        def reads [shape: string] {
            let texts = [string path glob external-argument signature external-signature]
            if $shape in ['any' 'list<any>' 'table'] or ($shape | str starts-with 'table<') {
                'L'
            } else if ($shape | str starts-with 'list<') {
                'O'
            } else if $shape in $texts {
                'text'
            } else {
                'none'
            }
        }
        def multispan [shape: string] {
            let spans = [condition expression signature external-signature vardecl math]
            if ($shape | str starts-with 'oneof<') {
                members $shape | any {|m| multispan $m }
            } else {
                ($shape | str starts-with '"') or $shape in $spans
            }
        }
        def negative [shape: string] {
            if ($shape | str starts-with 'oneof<') {
                members $shape | any {|m| negative $m }
            } else {
                $shape in [int number float]
            }
        }
        def closes [shape: string] {
            (members $shape | first) =~ '^(closure|block)'
        }
        def code [shape: string] {
            let reads = members $shape | each {|m| reads $m } | where $it != 'none'
            let falls_to_text = 'text' in ($reads | skip 1)
            let takes = match ($reads | get 0?) {
                'L' | 'O' if not $falls_to_text => $reads.0,
                _ => 'W',
            }
            let negative = if (negative $shape) { '-' } else { '' }
            let closure = if (closes $shape) { 'C' } else { '' }
            $takes + $negative + $closure
        }
        def codes [params: table] {
            $params | each {|p| code $p.syntax_shape } | str join ','
        }
        def flags [params: table] {
            $params | where parameter_type in [named switch] | each {|f|
                let short = if $f.short_flag == null { '' } else { $"/($f.short_flag)" }
                let value = if $f.parameter_type == named { $"=(code $f.syntax_shape)" } else { '' }
                $"($f.parameter_name)($short)($value)"
            } | str join ','
        }
        let parser_keywords = help commands | get name | where {|n|
            let error = ast $"def \"($n)\" [] {}" --json | get error | from json | default {}
            ($error | columns | get -o 0) == "NameIsKeyword"
        }
        let commands = scope commands
            | where type in [built-in keyword] and name not-in $parser_keywords
        let lines = $commands | each {|c|
            let params = $c.signatures | values | first | where parameter_type not-in [input output]
            let positional = $params | where parameter_type in [positional rest]
            let valued = $params | where parameter_type in [positional rest named]
            let lists = $valued | any {|p| (code $p.syntax_shape) =~ '^[LO]' }
            let closures = $valued | any {|p| closes $p.syntax_shape }
            if ($lists or $closures) and not ($positional | any {|p| multispan $p.syntax_shape }) {
                let positional = $params | where parameter_type == positional
                let required = codes ($positional | where not is_optional)
                let optional = codes ($positional | where is_optional)
                let rest = codes ($params | where parameter_type == rest)
                let line = $"($c.name)|($required)|($optional)|($rest)|(flags $params)"
                {name: $c.name, line: $line}
            }
        } | compact
        let names = $lines | get name
        let shadowing = scope commands | get name | where {|n|
            $n not-in $names and ($names | any {|x| $n | str starts-with $"($x) " })
        }
        $lines | get line | append $shadowing | sort | str join "\n"
    "#;

    fn code(param: Param) -> String {
        let takes = match param.takes {
            Takes::List => "L",
            Takes::ListOf => "O",
            Takes::Word => "W",
        };
        let negative = if param.negative { "-" } else { "" };
        let closure = if param.closure { "C" } else { "" };
        format!("{takes}{negative}{closure}")
    }

    fn line(command: &Command) -> String {
        let name = command.name.join(" ");
        let Some(signature) = &command.signature else {
            return name;
        };
        let codes = |params: &[Param]| {
            params
                .iter()
                .map(|&param| code(param))
                .collect::<Vec<_>>()
                .join(",")
        };
        let flags = signature
            .flags
            .iter()
            .map(|flag| {
                let short = flag.short.map(|short| format!("/{short}"));
                let value = flag.value.map(|value| format!("={}", code(value)));
                format!(
                    "{}{}{}",
                    flag.long,
                    short.unwrap_or_default(),
                    value.unwrap_or_default()
                )
            })
            .collect::<Vec<_>>()
            .join(",");
        let rest = signature.rest.map(code).unwrap_or_default();
        format!(
            "{name}|{}|{}|{rest}|{flags}",
            codes(signature.required),
            codes(signature.optional)
        )
    }

    #[test]
    fn commands_are_what_nu_declares() {
        let Some(declared) = asked(COMMANDS_OF_NU) else {
            return;
        };
        let mut lines: Vec<_> = COMMANDS.iter().map(line).collect();
        lines.sort();
        assert_eq!(lines, declared);
    }

    #[test]
    fn commands_are_sorted_without_repeats() {
        assert!(COMMANDS.windows(2).all(|pair| pair[0].name < pair[1].name));
    }

    #[test]
    fn positional_arguments_take_parameters_then_rest() {
        // `append` takes a row, then any number more.
        assert_eq!(
            parameters(&["append", "[a]", "[b]", "[c]"]),
            [None, Some(LIST), Some(LIST), Some(LIST)]
        );
        // `default` takes a value, then any number of column names as text.
        assert_eq!(
            parameters(&["default", "[a]", "[b]"]),
            [None, Some(LIST), Some(WORD)]
        );
    }

    #[test]
    fn name_words_take_no_parameter() {
        assert_eq!(
            parameters(&["http", "post", "u", "[a]"]),
            [None, None, Some(WORD), Some(LIST)]
        );
    }

    #[test]
    fn longer_command_name_replaces_shorter() {
        assert_eq!(
            parameters(&["ansi", "gradient", "[a]"]),
            [None, None, Some(WORD)]
        );
        assert_eq!(parameters(&["ansi", "[a]"]), [None, Some(LIST)]);
    }

    #[test]
    fn items_after_unlisted_head_take_word() {
        assert_eq!(parameters(&["ls", "[a]"]), [None, Some(WORD)]);
        assert_eq!(
            parameters(&["str", "join", "[a]"]),
            [None, Some(WORD), Some(WORD)]
        );
        assert_eq!(parameters(&["ls"]), [None]);
        assert_eq!(parameters(&[]), []);
    }

    #[test]
    fn every_item_of_parser_keyword_takes_nothing() {
        assert_eq!(parameters(&["def", "q", "[]", "{}", "{}"]), [None; 5]);
        assert_eq!(parameters(&["plugin", "use", "{}"]), [None; 3]);
        // Nu runs the command `overlay list`, but its first word leaves it unknown here.
        assert_eq!(parameters(&["overlay", "list", "{}"]), [None; 3]);
    }

    #[test]
    fn closure_parameter_takes_closure() {
        assert_eq!(parameters(&["each", "{}"]), [None, Some(CLOSURE)]);
        // `rename` takes a closure only as the value of `--block`.
        assert_eq!(
            parameters(&["rename", "--block", "{}", "{}"]),
            [None, None, Some(CLOSURE), Some(WORD)]
        );
    }

    #[test]
    fn flag_takes_next_item_as_value() {
        assert_eq!(
            parameters(&["glob", "--exclude", "[a]", "*"]),
            [None, None, Some(LIST_OF), Some(WORD)]
        );
        assert_eq!(
            parameters(&["glob", "-e", "[a]", "*"]),
            [None, None, Some(LIST_OF), Some(WORD)]
        );
        assert_eq!(
            parameters(&["glob", "-De", "[a]", "*"]),
            [None, None, Some(LIST_OF), Some(WORD)]
        );
    }

    #[test]
    fn switch_takes_no_value() {
        assert_eq!(
            parameters(&["print", "-n", "[a]"]),
            [None, None, Some(LIST)]
        );
    }

    #[test]
    fn equals_joined_value_leaves_next_item_positional() {
        assert_eq!(
            parameters(&["glob", "--exclude=[a]", "[b]"]),
            [None, None, Some(WORD)]
        );
    }

    #[test]
    fn valued_flag_mid_cluster_stops_flags() {
        // Nu reports `-eD` and reads no value after it.
        assert_eq!(
            parameters(&["glob", "-eD", "[a]"]),
            [None, None, Some(WORD)]
        );
    }

    #[test]
    fn unknown_flag_takes_no_value() {
        assert_eq!(
            parameters(&["print", "--nosuch", "[a]"]),
            [None, None, Some(LIST)]
        );
        assert_eq!(
            parameters(&["print", "-z", "[a]"]),
            [None, None, Some(LIST)]
        );
    }

    #[test]
    fn items_after_double_dash_are_positional() {
        assert_eq!(
            parameters(&["print", "--", "-n", "[a]"]),
            [None, None, Some(LIST), Some(LIST)]
        );
    }

    #[test]
    fn negative_number_fills_only_number_parameter() {
        // `matrix set-col` takes an index, then a list.
        assert_eq!(
            parameters(&["matrix", "set-col", "-1", "[a]"]),
            [None, None, Some(NUMBER), Some(LIST_OF)]
        );
        assert_eq!(
            parameters(&["append", "-1", "[a]"]),
            [None, None, Some(LIST)]
        );
    }

    #[test]
    fn spread_moves_to_rest_after_required_parameters() {
        // `do` takes a closure, then any number of arguments.
        assert_eq!(
            parameters(&["do", "{}", "...[1]", "[a]"]),
            [None, Some(CLOSURE), None, Some(LIST)]
        );
        // A spread before the closure fills nothing.
        assert_eq!(
            parameters(&["do", "...[1]", "[a]"]),
            [None, None, Some(CLOSURE)]
        );
    }

    #[test]
    fn spread_without_rest_fills_nothing() {
        // `ansi` takes one optional value and no rest.
        // Nu rejects the spread.
        assert_eq!(
            parameters(&["ansi", "...$x", "[a]"]),
            [None, None, Some(LIST)]
        );
    }

    #[test]
    fn extra_argument_takes_nothing() {
        assert_eq!(
            parameters(&["difference", "[a]", "[b]"]),
            [None, Some(LIST), None]
        );
    }

    #[test]
    fn type_takes_list_where_nu_reads_one() {
        for ty in [
            "any",
            "list",
            "list<any>",
            "list< any >",
            "table",
            "table<a: int>",
        ] {
            assert_eq!(param_of_type(ty), Some(LIST), "{ty:?}");
        }
        assert_eq!(param_of_type("list<string>"), Some(LIST_OF));
        assert_eq!(param_of_type("oneof<record, list<any>>"), Some(LIST));
        assert_eq!(param_of_type("oneof<list<any>, table>"), Some(LIST));
    }

    #[test]
    fn type_takes_word_where_nu_reads_none() {
        for ty in [
            "string",
            "path",
            "glob",
            "string@complete",
            "record<a: int>",
        ] {
            assert_eq!(param_of_type(ty), Some(WORD), "{ty:?}");
        }
        assert_eq!(param_of_type("oneof<string, list<any>>"), Some(WORD));
    }

    #[test]
    fn oneof_list_before_text_takes_word() {
        // Nu reads the `[ ]` as text where the list has an error it does not report.
        assert_eq!(param_of_type("oneof<list<any>, string>"), Some(WORD));
    }

    #[test]
    fn oneof_members_part_at_whitespace_and_comments_as_at_commas() {
        for ty in [
            "oneof<list<any> string>",
            "oneof<list<any>\tstring>",
            "oneof< list<any> ,, string >",
            "oneof<list<any> # c\n string>",
        ] {
            assert_eq!(param_of_type(ty), Some(WORD), "{ty:?}");
        }
        assert_eq!(param_of_type("oneof<record list<any>>"), Some(LIST));
    }

    #[test]
    fn number_type_takes_negative_number() {
        for ty in ["int", "float", "number", "oneof<int, list<any>>"] {
            assert!(param_of_type(ty).unwrap().negative, "{ty:?}");
        }
    }

    #[test]
    fn type_starting_with_closure_takes_closure() {
        for ty in [
            "closure",
            "oneof<closure, string>",
            "oneof< closure , list<any>>",
        ] {
            assert!(param_of_type(ty).unwrap().closure, "{ty:?}");
        }
        // Nu reads a record first, then rejects it by its type.
        assert!(!param_of_type("oneof<string, closure>").unwrap().closure);
    }

    #[test]
    fn unknown_type_takes_nothing() {
        for ty in [
            "nosuch",
            "oneof<>",
            "oneof<nosuch, string>",
            "closure(int)",
            "oneof<closure(int), string>",
            "oneof<int:string>",
        ] {
            assert_eq!(param_of_type(ty), None, "{ty:?}");
        }
    }

    fn declared(items: &[Item<'static>]) -> Option<Declared<'static>> {
        Declared::of(items.iter().copied())
    }

    #[test]
    fn arguments_take_declared_parameter_types() {
        use Item::{Colon, Eq, Word};
        let signature = declared(&[
            Word("a"),
            Word("b"),
            Colon,
            Word("string"),
            Word("c?"),
            Colon,
            Word("list<string>"),
            Word("--flag"),
            Word("(-f)"),
            Colon,
            Word("list"),
            Word("--switch"),
            Word("...rest"),
            Colon,
            Word("int"),
        ])
        .unwrap();
        assert_eq!(
            signature.arguments(&["[a]", "--switch", "[b]", "-f", "[c]", "[d]", "[e]"]),
            [
                Some(LIST),
                None,
                Some(WORD),
                None,
                Some(LIST),
                Some(LIST_OF),
                Some(NUMBER)
            ]
        );
        let flag_with_default = declared(&[Word("--d"), Eq, Word("1")]).unwrap();
        assert_eq!(
            flag_with_default.arguments(&["--d", "[a]"]),
            [None, Some(WORD)]
        );
    }

    #[test]
    fn record_spread_fills_no_positional_parameter() {
        use Item::{Colon, Word};
        let signature = declared(&[
            Word("a"),
            Colon,
            Word("string"),
            Word("b"),
            Colon,
            Word("list"),
            Word("--flag"),
        ])
        .unwrap();
        assert_eq!(
            signature.arguments(&["...{flag: true}", "x", "[b]"]),
            [None, Some(WORD), Some(LIST)]
        );
        // Nu rejects a `{ }` spread without flags.
        // It fills no positional parameter there either.
        let with_rest = declared(&OPTIONAL_LIST_THEN_REST).unwrap();
        assert_eq!(
            with_rest.arguments(&["...{a: 1}", "[b]"]),
            [None, Some(LIST)]
        );
    }

    const OPTIONAL_LIST_THEN_REST: [Item<'static>; 6] = [
        Item::Word("a?"),
        Item::Colon,
        Item::Word("list"),
        Item::Word("...r"),
        Item::Colon,
        Item::Word("int"),
    ];

    fn with_flag(flag: &'static str) -> Declared<'static> {
        declared(&[&OPTIONAL_LIST_THEN_REST[..], &[Item::Word(flag)]].concat()).unwrap()
    }

    #[test]
    fn variable_or_subexpression_spread_fills_parameters_up_to_rest_only_without_flags() {
        let without_flags = declared(&OPTIONAL_LIST_THEN_REST).unwrap();
        for spread in ["...$x", "...($x)"] {
            assert_eq!(
                without_flags.arguments(&[spread, "[b]"]),
                [None, Some(NUMBER)],
                "{spread}"
            );
            assert_eq!(
                with_flag("--flag").arguments(&[spread, "[b]"]),
                [None, Some(LIST)],
                "{spread}"
            );
        }
    }

    #[test]
    fn declared_help_flag_counts_as_no_flag_for_spread() {
        assert_eq!(
            with_flag("--help").arguments(&["...$x", "[b]"]),
            [None, Some(NUMBER)]
        );
    }

    #[test]
    fn list_spread_fills_parameters_up_to_rest_even_with_flags() {
        assert_eq!(
            with_flag("--flag").arguments(&["...[1]", "[b]"]),
            [None, Some(NUMBER)]
        );
    }

    #[test]
    fn untyped_default_gives_type_of_value() {
        use Item::{Eq, Value, Word};
        let number = declared(&[Word("b"), Eq, Word("1"), Word("...r")]).unwrap();
        assert_eq!(number.arguments(&["-1", "[a]"]), [Some(NUMBER), Some(LIST)]);
        let string = declared(&[Word("b"), Eq, Value]).unwrap();
        assert_eq!(string.arguments(&["[a]"]), [Some(WORD)]);
    }

    #[test]
    fn untyped_default_nuten_cannot_type_declares_nothing() {
        use Item::{Eq, Word};
        for default in ["[1]", "{a: 1}", "(1)", "$x", "null"] {
            assert!(
                declared(&[Word("b"), Eq, Word(default)]).is_none(),
                "{default:?}"
            );
        }
    }

    #[test]
    fn unreadable_signature_declares_nothing() {
        use Item::{Colon, Eq, Other, Word};
        for items in [
            vec![Word("a"), Other],
            vec![Word("a"), Colon],
            vec![Word("a"), Eq],
            vec![Colon, Word("int")],
            vec![Word("a"), Word("(-f)")],
            vec![Word("--flag"), Colon, Word("bool")],
            vec![Word("a?"), Word("b")],
            vec![Word("...r"), Word("a")],
            vec![Word("...r"), Word("...s")],
            vec![Word("[a]")],
        ] {
            assert!(declared(&items).is_none(), "{items:?}");
        }
    }
}
