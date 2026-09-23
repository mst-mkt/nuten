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

/// A name of a variable stands where these take their first argument, never a value.
pub(super) fn takes_variable_name(head: &str) -> bool {
    matches!(head, "let" | "mut" | "const")
}

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
    use std::process::Command;

    use super::{
        COMMANDS, EXTERNAL_AFTER_PERCENT, is_command, reads_before_assignment,
        resolves_after_percent, takes_any_argument, takes_row_condition,
    };

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

    /// `None` when nu cannot be run, so that a machine without it still runs every other test.
    /// Nu failing means the pipeline no longer asks what it meant to, which must not pass.
    fn asked(pipeline: &str) -> Option<Vec<String>> {
        let output = Command::new("nu")
            .args(["-n", "-c", pipeline])
            .output()
            .ok()?;
        assert!(
            output.status.success(),
            "{pipeline}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        Some(
            String::from_utf8_lossy(&output.stdout)
                .lines()
                .map(str::to_owned)
                .collect(),
        )
    }

    #[test]
    fn lists_are_sorted_without_repeats() {
        for list in [COMMANDS, EXTERNAL_AFTER_PERCENT] {
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
}
