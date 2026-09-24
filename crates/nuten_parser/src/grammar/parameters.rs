//! The parameters of the commands nu 0.115.1 has, as far as they tell how nu reads a `[ ]`.
//!
//! Only a command with a parameter that takes a `[ ]` as a list is here.
//! Every other one takes it as text or rejects it, so it stays a word wherever it stands.
//! A command nu reads a keyword syntax of is left out too, since its own rule reads it.
//! None of these takes arguments it does not declare.

use std::iter;

use super::spread::{Target, is_spread};

/// How nu reads a `[ ]` where it takes a parameter.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Takes {
    /// A list or a table, with each item any value, as for `any`, `list<any>`, or `table`.
    List,
    /// A list with items of another type, as for `list<string>`.
    /// An item nu then takes as text or rejects, such as a `[ ]` or a `{ }`, stays a word.
    /// A table stays a word too.
    ListOf,
    /// Text, or no value: nu reads the `[ ]` as one string, or rejects it.
    Word,
}

/// What one parameter takes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Param {
    pub(super) takes: Takes,
    /// Whether it takes a negative number, which an item like `-1` then stands for.
    negative: bool,
}

const fn param(takes: Takes, negative: bool) -> Param {
    Param {
        takes,
        negative,
    }
}

const LIST: Param = param(Takes::List, false);
const LIST_OF: Param = param(Takes::ListOf, false);
const WORD: Param = param(Takes::Word, false);
const NUMBER: Param = param(Takes::Word, true);

/// A flag, with what it takes after it if anything.
#[derive(Debug)]
struct Flag {
    long: &'static str,
    short: Option<char>,
    value: Option<Param>,
}

const fn flag(long: &'static str, short: Option<char>, value: Option<Param>) -> Flag {
    Flag {
        long,
        short,
        value,
    }
}

/// The parameters of a command, in nu's order.
#[derive(Debug)]
struct Signature {
    required: &'static [Param],
    optional: &'static [Param],
    rest: Option<Param>,
    flags: &'static [Flag],
}

impl Signature {
    /// The positional parameter at `index`, the rest taking every one past the others.
    fn positional(&self, index: usize) -> Option<Param> {
        self.required
            .iter()
            .chain(self.optional)
            .nth(index)
            .copied()
            .or(self.rest)
    }

    fn long(&self, name: &str) -> Option<&Flag> {
        self.flags.iter().find(|flag| flag.long == name)
    }

    fn short(&self, name: char) -> Option<&Flag> {
        self.flags.iter().find(|flag| flag.short == Some(name))
    }
}

/// A command by the words of its name.
#[derive(Debug)]
struct Command {
    name: &'static [&'static str],
    /// `None` for one with no parameter here, which takes the place of a shorter name that has one.
    signature: Option<Signature>,
}

const fn command(name: &'static [&'static str], signature: Option<Signature>) -> Command {
    Command {
        name,
        signature,
    }
}

/// What nu takes each of `items` as, the head first, where it runs a command nu has.
/// `None` for an item that is no parameter, such as a word of the name or a flag.
/// Also for every item when the command is none here.
/// Nu runs the command of the longest name the items spell.
pub(super) fn parameters(items: &[&str]) -> Vec<Option<Param>> {
    let Some(command) = longest_spelled(items) else {
        return vec![None; items.len()];
    };
    let words = command.name.len();
    let arguments = command.signature.as_ref().map_or_else(
        || vec![None; items.len() - words],
        |signature| arguments(signature, &items[words..]),
    );
    iter::repeat_n(None, words).chain(arguments).collect()
}

/// The command of the longest name `items` start with.
fn longest_spelled(items: &[&str]) -> Option<&'static Command> {
    (1..=items.len()).rev().find_map(|len| {
        COMMANDS
            .binary_search_by(|command| command.name.cmp(&items[..len]))
            .ok()
            .map(|nth| &COMMANDS[nth])
    })
}

/// What nu takes each of `items` as, the arguments of a call of `signature`.
/// Ported from the loop of nu's `parse_internal_call` that goes through the arguments.
fn arguments(signature: &Signature, items: &[&str]) -> Vec<Option<Param>> {
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
        } else if is_spread(item, Target::List) {
            // A spread fills the positional parameters up to the rest, where nu takes one.
            if signature.rest.is_some() && positional >= signature.required.len() {
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

/// The flags `item` stands for, as nu's `parse_short_flags` reads it.
/// `None` where it is no flag: an item that is no `-` and more, or a negative number nu takes.
/// Only the last flag may take a value; nu stops at one before it that does.
fn short_flags<'s>(
    signature: &'s Signature,
    item: &str,
    positional: usize,
) -> Option<Vec<&'s Flag>> {
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
            required: &[WORD, WORD],
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
            required: &[WORD],
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
        &["echo"],
        Some(Signature {
            required: &[],
            optional: &[],
            rest: Some(LIST),
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
            required: &[WORD],
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
        &["intersect"],
        Some(Signature {
            required: &[LIST],
            optional: &[],
            rest: None,
            flags: &[],
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
            required: &[WORD],
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
    command(&["metadata", "access"], None),
    command(
        &["metadata", "set"],
        Some(Signature {
            required: &[],
            optional: &[WORD],
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
            required: &[WORD],
            optional: &[],
            rest: None,
            flags: &[flag("fold", Some('f'), Some(LIST))],
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
            required: &[WORD],
            optional: &[],
            rest: None,
            flags: &[flag("columns", Some('c'), Some(LIST))],
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
            flags: &[],
        }),
    ),
    command(
        &["with-env"],
        Some(Signature {
            required: &[LIST, WORD],
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
    use super::{COMMANDS, Command, LIST, LIST_OF, NUMBER, Param, Takes, WORD, parameters};
    use crate::test_support::asked;

    /// Each command nu has with a parameter that takes a `[ ]` as a list, in the table's lines.
    /// Then each name that extends one of theirs, alone on its line.
    /// A type reads a `[ ]` as nu's `parse_value` does.
    /// A `oneof` reads it as its first type that takes one.
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
        def code [shape: string] {
            let reads = members $shape | each {|m| reads $m } | where $it != 'none'
            let falls_to_text = 'text' in ($reads | skip 1)
            let takes = match ($reads | get 0?) {
                'L' | 'O' if not $falls_to_text => $reads.0,
                _ => 'W',
            }
            $takes + (if (negative $shape) { '-' } else { '' })
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
        let lines = scope commands | where type == built-in | each {|c|
            let params = $c.signatures | values | first | where parameter_type not-in [input output]
            let positional = $params | where parameter_type in [positional rest]
            let valued = $params | where parameter_type in [positional rest named]
            let lists = $valued | any {|p| (code $p.syntax_shape) =~ '^[LO]' }
            if $lists and not ($positional | any {|p| multispan $p.syntax_shape }) {
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
        format!("{takes}{negative}")
    }

    /// The line `COMMANDS_OF_NU` gives the same command.
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
    fn positional_arguments_take_parameters_in_order_then_rest() {
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
    fn name_of_several_words_takes_no_parameter() {
        assert_eq!(
            parameters(&["http", "post", "u", "[a]"]),
            [None, None, Some(WORD), Some(LIST)]
        );
    }

    #[test]
    fn longer_name_of_command_with_no_list_parameter_takes_place_of_shorter_one() {
        assert_eq!(parameters(&["ansi", "gradient", "[a]"]), [None; 3]);
        assert_eq!(parameters(&["ansi", "[a]"]), [None, Some(LIST)]);
    }

    #[test]
    fn command_not_here_takes_nothing() {
        assert_eq!(parameters(&["ls", "[a]"]), [None; 2]);
        assert_eq!(parameters(&["str", "join", "[a]"]), [None; 3]);
        assert_eq!(parameters(&[]), []);
    }

    #[test]
    fn flag_takes_item_after_it_as_its_value() {
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
    fn value_joined_to_flag_by_equals_sign_leaves_next_item_positional() {
        assert_eq!(
            parameters(&["glob", "--exclude=[a]", "[b]"]),
            [None, None, Some(WORD)]
        );
    }

    #[test]
    fn short_flag_that_takes_value_before_last_one_stops_flags() {
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
    fn negative_number_is_positional_where_parameter_takes_number() {
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
            parameters(&["do", "{}", "...$x", "[a]"]),
            [None, Some(WORD), None, Some(LIST)]
        );
        // A spread before the closure fills nothing.
        assert_eq!(
            parameters(&["do", "...$x", "[a]"]),
            [None, None, Some(WORD)]
        );
    }

    #[test]
    fn spread_into_command_with_no_rest_fills_nothing() {
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
}
