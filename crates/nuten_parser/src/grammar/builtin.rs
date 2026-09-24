//! The names nu 0.115.1 resolves to a command of its own.
//!
//! Nu registers the parent of a subcommand as a command too, so `str` stands for `str trim`.
//! A plugin is left out: it is registered per user, and nu runs an unregistered one externally.

pub(super) fn is_command(head: &str) -> bool {
    COMMANDS.binary_search(&head).is_ok()
}

pub(super) fn resolves_after_percent(head: &str) -> bool {
    is_command(head) && EXTERNAL_AFTER_PERCENT.binary_search(&head).is_err()
}

pub(super) fn takes_row_condition(head: &str) -> bool {
    matches!(head, "all" | "any" | "where")
}

/// Nu takes any value for the argument of these.
/// Unlike a command, a keyword cannot be defined again, so its signature is known here.
pub(super) fn takes_any_argument(head: &str) -> bool {
    head == "return"
}

/// Whether nu reads a statement headed by `head`, then `next`, as the syntax of that keyword.
/// Nu reads such a keyword before an assignment operator in the statement.
pub(super) fn reads_before_assignment(head: &str, next: Option<&str>) -> bool {
    matches!(
        head,
        "alias"
            | "const"
            | "def"
            | "export"
            | "export-env"
            | "extern"
            | "for"
            | "hide"
            | "let"
            | "module"
            | "mut"
            | "overlay"
            | "run"
            | "source"
            | "source-env"
            | "use"
            | "where"
    ) || (head == "plugin" && next == Some("use"))
}

/// Whether nu rejects `name` as the name of a command a `def` or an `extern` defines.
/// Ported from nu's `is_parser_keyword`.
pub(super) fn is_parser_keyword(name: &str) -> bool {
    matches!(
        name,
        "if" | "match"
            | "try"
            | "overlay"
            | "overlay hide"
            | "overlay new"
            | "overlay use"
            | "alias"
            | "const"
            | "def"
            | "extern"
            | "module"
            | "use"
            | "export"
            | "export alias"
            | "export const"
            | "export def"
            | "export extern"
            | "export module"
            | "export use"
            | "for"
            | "loop"
            | "while"
            | "return"
            | "break"
            | "continue"
            | "let"
            | "mut"
            | "hide"
            | "export-env"
            | "source-env"
            | "source"
            | "run"
            | "where"
            | "plugin use"
    )
}

/// Whether a name whose first words are `head` and `next` starts with a parser keyword.
/// Every keyword of two words but `plugin use` starts with one that is a keyword alone.
pub(super) fn starts_with_parser_keyword(head: &str, next: Option<&str>) -> bool {
    is_parser_keyword(head) || (head == "plugin" && next == Some("use"))
}

/// A name of a variable stands where these take their first argument, never a value.
pub(super) fn takes_variable_name(head: &str) -> bool {
    matches!(head, "let" | "mut" | "const")
}

/// Whether nu resolves the attribute `@name` to one of its own, a command `attr name`.
pub(super) fn is_attribute(name: &str) -> bool {
    ATTRIBUTES.binary_search(&name).is_ok()
}

/// The words after `attr` in the names of nu's attributes.
/// Sorted, so that `is_attribute` can bisect.
const ATTRIBUTES: &[&str] = &[
    "category",
    "complete",
    "deprecated",
    "example",
    "search-terms",
];

/// Sorted, so that `resolves_after_percent` can bisect.
/// No rule tells these apart from the rest, so the test asks nu which they are.
const EXTERNAL_AFTER_PERCENT: &[&str] = &[
    "alias",
    "banner",
    "break",
    "collect",
    "const",
    "continue",
    "def",
    "export",
    "export-env",
    "extern",
    "for",
    "hide",
    "if",
    "let",
    "loop",
    "match",
    "module",
    "mut",
    "overlay",
    "pwd",
    "return",
    "run",
    "source",
    "source-env",
    "try",
    "use",
    "where",
    "while",
];

/// Sorted, so that `is_command` can bisect.
const COMMANDS: &[&str] = &[
    "abbr",
    "alias",
    "all",
    "ansi",
    "any",
    "append",
    "ast",
    "attr",
    "banner",
    "bits",
    "break",
    "bytes",
    "cal",
    "cd",
    "char",
    "chunk-by",
    "chunks",
    "clear",
    "collect",
    "columns",
    "combinations",
    "commandline",
    "compact",
    "complete",
    "config",
    "const",
    "continue",
    "cp",
    "date",
    "debug",
    "decode",
    "def",
    "default",
    "describe",
    "detect",
    "difference",
    "do",
    "drop",
    "du",
    "each",
    "echo",
    "encode",
    "enumerate",
    "error",
    "every",
    "exec",
    "exit",
    "explain",
    "explore",
    "export",
    "export-env",
    "extern",
    "fill",
    "filter",
    "find",
    "first",
    "flatten",
    "for",
    "format",
    "from",
    "generate",
    "get",
    "glob",
    "grid",
    "group-by",
    "hash",
    "headers",
    "help",
    "hide",
    "hide-env",
    "histogram",
    "history",
    "http",
    "idx",
    "if",
    "ignore",
    "input",
    "insert",
    "inspect",
    "interleave",
    "intersect",
    "into",
    "is-admin",
    "is-empty",
    "is-not-empty",
    "is-redirected",
    "is-terminal",
    "items",
    "job",
    "join",
    "keybindings",
    "kill",
    "last",
    "length",
    "let",
    "let-env",
    "lines",
    "load-env",
    "loop",
    "ls",
    "match",
    "math",
    "matrix",
    "merge",
    "metadata",
    "mkdir",
    "mktemp",
    "module",
    "move",
    "mut",
    "mv",
    "nu-check",
    "nu-highlight",
    "open",
    "overlay",
    "panic",
    "par-each",
    "parse",
    "path",
    "peek",
    "permutations",
    "plugin",
    "port",
    "prepend",
    "print",
    "ps",
    "pwd",
    "query",
    "random",
    "reduce",
    "reject",
    "rename",
    "return",
    "reverse",
    "rm",
    "roll",
    "rotate",
    "run",
    "run-external",
    "run-internal",
    "save",
    "schema",
    "scope",
    "select",
    "semver",
    "seq",
    "shuffle",
    "skip",
    "sleep",
    "slice",
    "sort",
    "sort-by",
    "source",
    "source-env",
    "split",
    "start",
    "stor",
    "str",
    "sys",
    "table",
    "take",
    "tee",
    "term",
    "timeit",
    "to",
    "touch",
    "transpose",
    "try",
    "tutor",
    "ulimit",
    "umask",
    "uname",
    "union",
    "uniq",
    "uniq-by",
    "unlet",
    "update",
    "upsert",
    "url",
    "use",
    "values",
    "version",
    "view",
    "watch",
    "where",
    "which",
    "while",
    "whoami",
    "window",
    "with-env",
    "wrap",
    "zip",
];

#[cfg(test)]
mod tests {
    use super::{
        ATTRIBUTES, COMMANDS, EXTERNAL_AFTER_PERCENT, is_command, is_parser_keyword,
        reads_before_assignment, resolves_after_percent, starts_with_parser_keyword,
        takes_any_argument, takes_row_condition,
    };
    use crate::test_support::asked;

    const ATTRIBUTES_OF_NU: &str = r#"help commands | get name
        | where { |n| $n | str starts-with "attr " }
        | each { |n| $n | split row " " | get 1 } | uniq | sort | str join "\n""#;

    const COMMANDS_OF_NU: &str = r#"help commands | get name
        | each { |n| $n | split row " " | first } | uniq | sort | str join "\n""#;

    const EXTERNAL_AFTER_PERCENT_OF_NU: &str = r#"help commands | get name
        | each { |n| $n | split row " " | first } | uniq | sort
        | where { |c| (ast --flatten $"%($c) x" | get 0.shape) == "shape_external" }
        | str join "\n""#;

    const ROW_CONDITIONS_OF_NU: &str = r#"scope commands
        | where { |c| $c.signatures | values | flatten
            | any { |p| ($p.syntax_shape | default "" | into string) =~ "condition" } }
        | get name | sort | str join "\n""#;

    const ANY_ARGUMENT_OF_NU: &str = r#"scope commands | where type == keyword
        | where { |c| ($c.signatures | values | flatten
            | where parameter_type in [positional rest] | get syntax_shape) == [any] }
        | get name | sort | str join "\n""#;

    const NAMES_OF_NU: &str = r#"help commands | get name | sort | str join "\n""#;

    const BEFORE_ASSIGNMENT_OF_NU: &str = r#"help commands | get name
        | where { |n| (ast $"($n) -h = 1" --json | get error | from json | default {}
            | columns | get -o 0) != "AssignmentRequiresVar" }
        | sort | str join "\n""#;

    const KEYWORD_NAMES_OF_NU: &str = r#"help commands | get name
        | where { |n| (ast $"def \"($n)\" [] {}" --json | get error | from json | default {}
            | columns | get -o 0) == "NameIsKeyword" }
        | sort | str join "\n""#;

    #[test]
    fn lists_are_sorted_without_repeats() {
        for list in [ATTRIBUTES, COMMANDS, EXTERNAL_AFTER_PERCENT] {
            assert!(list.windows(2).all(|pair| pair[0] < pair[1]), "{list:?}");
        }
    }

    #[test]
    fn every_name_after_percent_is_a_command() {
        assert!(EXTERNAL_AFTER_PERCENT.iter().all(|name| is_command(name)));
    }

    #[test]
    fn commands_are_what_nu_has() {
        let Some(names) = asked(COMMANDS_OF_NU) else {
            return;
        };
        assert_eq!(COMMANDS, names);
    }

    #[test]
    fn attributes_are_what_nu_has() {
        let Some(names) = asked(ATTRIBUTES_OF_NU) else {
            return;
        };
        assert_eq!(ATTRIBUTES, names);
    }

    #[test]
    fn externals_are_what_nu_leaves_after_percent() {
        let Some(names) = asked(EXTERNAL_AFTER_PERCENT_OF_NU) else {
            return;
        };
        let external: Vec<&str> = COMMANDS
            .iter()
            .copied()
            .filter(|head| !resolves_after_percent(head))
            .collect();
        assert_eq!(external, names);
    }

    #[test]
    fn row_conditions_are_what_nu_declares() {
        let Some(names) = asked(ROW_CONDITIONS_OF_NU) else {
            return;
        };
        let with_condition: Vec<&str> = COMMANDS
            .iter()
            .copied()
            .filter(|head| takes_row_condition(head))
            .collect();
        assert_eq!(with_condition, names);
    }

    #[test]
    fn keywords_taking_any_argument_are_what_nu_declares() {
        let Some(names) = asked(ANY_ARGUMENT_OF_NU) else {
            return;
        };
        let with_any: Vec<&str> = COMMANDS
            .iter()
            .copied()
            .filter(|head| takes_any_argument(head))
            .collect();
        assert_eq!(with_any, names);
    }

    #[test]
    fn keywords_read_before_assignment_are_what_nu_dispatches_on_first() {
        let (Some(names), Some(before)) = (asked(NAMES_OF_NU), asked(BEFORE_ASSIGNMENT_OF_NU))
        else {
            return;
        };
        let read_before: Vec<&str> = names
            .iter()
            .map(String::as_str)
            .filter(|name| match name.split_once(' ') {
                Some((head, next)) => reads_before_assignment(head, Some(next)),
                None => reads_before_assignment(name, None),
            })
            .collect();
        assert_eq!(read_before, before);
    }

    #[test]
    fn parser_keywords_are_names_nu_rejects_for_def() {
        let (Some(names), Some(keywords)) = (asked(NAMES_OF_NU), asked(KEYWORD_NAMES_OF_NU)) else {
            return;
        };
        let rejected: Vec<&str> = names
            .iter()
            .map(String::as_str)
            .filter(|name| is_parser_keyword(name))
            .collect();
        assert_eq!(rejected, keywords);
    }

    #[test]
    fn every_parser_keyword_starts_with_parser_keyword() {
        let Some(keywords) = asked(KEYWORD_NAMES_OF_NU) else {
            return;
        };
        for keyword in &keywords {
            let (head, next) = match keyword.split_once(' ') {
                Some((head, next)) => (head, Some(next)),
                None => (keyword.as_str(), None),
            };
            assert!(starts_with_parser_keyword(head, next), "{keyword:?}");
        }
    }
}
