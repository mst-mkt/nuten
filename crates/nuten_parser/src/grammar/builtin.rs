//! Nu registers the parent of a subcommand as a command too, so `str` stands for `str trim`.
//! A plugin is left out: it is registered per user, and nu runs an unregistered one externally.
//! A command nu has only on some platforms is taken in, since a file may run on another one.

pub fn is_command(head: &str) -> bool {
    COMMANDS.binary_search(&head).is_ok()
}

pub(super) fn name_words(words: &[&str]) -> usize {
    (2..=words.len().min(LONGEST_NAME))
        .rev()
        .find(|&len| {
            MULTIWORD_NAMES
                .binary_search(&words[..len].join(" ").as_str())
                .is_ok()
        })
        .unwrap_or(1)
}

pub(super) fn resolves_after_percent(head: &str) -> bool {
    is_command(head) && EXTERNAL_AFTER_PERCENT.binary_search(&head).is_err()
}

pub(super) fn row_condition_words(head: &str, next: Option<&str>) -> Option<usize> {
    match (head, next) {
        ("all" | "any" | "chunk-by" | "where", _) => Some(1),
        ("skip" | "take", Some("until" | "while")) => Some(2),
        _ => None,
    }
}

/// Unlike a command, a keyword cannot be defined again, so its signature is known here.
pub(super) fn takes_any_argument(head: &str) -> bool {
    head == "return"
}

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

/// Every keyword of two words but `plugin use` starts with one that is a keyword alone.
pub(super) fn starts_with_parser_keyword(head: &str, next: Option<&str>) -> bool {
    is_parser_keyword(head) || (head == "plugin" && next == Some("use"))
}

pub(super) fn takes_variable_name(head: &str) -> bool {
    matches!(head, "let" | "mut" | "const")
}

pub(super) fn is_attribute(name: &str) -> bool {
    ATTRIBUTES.binary_search(&name).is_ok()
}

/// Sorted, so that `is_attribute` can bisect.
const ATTRIBUTES: &[&str] = &[
    "category",
    "complete",
    "deprecated",
    "example",
    "interactive",
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
    "registry",
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
    "tui",
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

const LONGEST_NAME: usize = 3;

/// Sorted, so that `name_words` can bisect.
const MULTIWORD_NAMES: &[&str] = &[
    "abbr list",
    "ansi gradient",
    "ansi link",
    "ansi strip",
    "attr category",
    "attr complete",
    "attr complete external",
    "attr deprecated",
    "attr example",
    "attr interactive",
    "attr search-terms",
    "bits and",
    "bits not",
    "bits or",
    "bits rol",
    "bits ror",
    "bits shl",
    "bits shr",
    "bits xor",
    "bytes add",
    "bytes at",
    "bytes build",
    "bytes collect",
    "bytes ends-with",
    "bytes index-of",
    "bytes length",
    "bytes remove",
    "bytes replace",
    "bytes reverse",
    "bytes split",
    "bytes starts-with",
    "commandline complete",
    "commandline edit",
    "commandline get-cursor",
    "commandline set-cursor",
    "commandline set-prompt",
    "config env",
    "config flatten",
    "config nu",
    "config reset",
    "config use-colors",
    "date format",
    "date from-human",
    "date humanize",
    "date list-timezone",
    "date now",
    "date to-timezone",
    "debug env",
    "debug experimental-options",
    "debug info",
    "debug profile",
    "decode base32",
    "decode base32hex",
    "decode base64",
    "decode hex",
    "detect columns",
    "detect type",
    "drop column",
    "drop nth",
    "each while",
    "encode base32",
    "encode base32hex",
    "encode base64",
    "encode hex",
    "error make",
    "explore config",
    "explore regex",
    "export alias",
    "export const",
    "export def",
    "export extern",
    "export module",
    "export use",
    "format bits",
    "format date",
    "format duration",
    "format filesize",
    "format number",
    "format pattern",
    "from csv",
    "from json",
    "from kdl",
    "from md",
    "from msgpack",
    "from msgpackz",
    "from nuon",
    "from ods",
    "from ssv",
    "from toml",
    "from tsv",
    "from url",
    "from xlsx",
    "from xml",
    "from yaml",
    "from yml",
    "hash md5",
    "hash sha256",
    "hash sha512",
    "help aliases",
    "help commands",
    "help escapes",
    "help externs",
    "help modules",
    "help operators",
    "help pipe-and-redirect",
    "history import",
    "history session",
    "http delete",
    "http get",
    "http head",
    "http options",
    "http patch",
    "http pool",
    "http post",
    "http put",
    "idx dirs",
    "idx drop",
    "idx files",
    "idx find",
    "idx init",
    "idx search",
    "idx status",
    "idx watch",
    "input list",
    "input listen",
    "into binary",
    "into bool",
    "into cell-path",
    "into datetime",
    "into duration",
    "into filesize",
    "into float",
    "into glob",
    "into int",
    "into matrix",
    "into record",
    "into semver",
    "into semver-range",
    "into sqlite",
    "into string",
    "into value",
    "job describe",
    "job flush",
    "job id",
    "job kill",
    "job list",
    "job recv",
    "job send",
    "job spawn",
    "job unfreeze",
    "keybindings default",
    "keybindings list",
    "keybindings listen",
    "math abs",
    "math arccos",
    "math arccosh",
    "math arcsin",
    "math arcsinh",
    "math arctan",
    "math arctanh",
    "math avg",
    "math cbrt",
    "math ceil",
    "math cos",
    "math cosh",
    "math exp",
    "math floor",
    "math ln",
    "math log",
    "math max",
    "math median",
    "math min",
    "math mode",
    "math product",
    "math round",
    "math sin",
    "math sinh",
    "math sqrt",
    "math stddev",
    "math sum",
    "math tan",
    "math tanh",
    "math variance",
    "matrix add",
    "matrix get-col",
    "matrix get-row",
    "matrix identity",
    "matrix into-nu",
    "matrix map",
    "matrix max",
    "matrix mean",
    "matrix multiply",
    "matrix reduce",
    "matrix reshape",
    "matrix scale",
    "matrix set-col",
    "matrix set-row",
    "matrix subtract",
    "matrix sum",
    "matrix transpose",
    "matrix zeros",
    "merge deep",
    "metadata access",
    "metadata set",
    "overlay hide",
    "overlay list",
    "overlay new",
    "overlay use",
    "path basename",
    "path dirname",
    "path exists",
    "path expand",
    "path join",
    "path parse",
    "path relative-to",
    "path self",
    "path split",
    "path type",
    "plugin add",
    "plugin list",
    "plugin rm",
    "plugin stop",
    "plugin use",
    "query db",
    "random binary",
    "random bool",
    "random chars",
    "random float",
    "random int",
    "random pass",
    "random uuid",
    "registry query",
    "roll down",
    "roll left",
    "roll right",
    "roll up",
    "scope aliases",
    "scope commands",
    "scope engine-stats",
    "scope externs",
    "scope modules",
    "scope variables",
    "semver bump",
    "seq char",
    "seq date",
    "skip until",
    "skip while",
    "split cell-path",
    "split chars",
    "split column",
    "split list",
    "split row",
    "split words",
    "stor create",
    "stor delete",
    "stor export",
    "stor import",
    "stor insert",
    "stor open",
    "stor reset",
    "stor update",
    "str camel-case",
    "str capitalize",
    "str contains",
    "str distance",
    "str downcase",
    "str ends-with",
    "str escape-regex",
    "str expand",
    "str index-of",
    "str join",
    "str kebab-case",
    "str length",
    "str lowercase",
    "str pascal-case",
    "str replace",
    "str reverse",
    "str screaming-snake-case",
    "str snake-case",
    "str starts-with",
    "str stats",
    "str substring",
    "str title-case",
    "str trim",
    "str upcase",
    "str uppercase",
    "sys cpu",
    "sys disks",
    "sys host",
    "sys mem",
    "sys net",
    "sys temp",
    "sys users",
    "take until",
    "take while",
    "term query",
    "term size",
    "to csv",
    "to html",
    "to json",
    "to kdl",
    "to md",
    "to msgpack",
    "to msgpackz",
    "to nuon",
    "to text",
    "to toml",
    "to tsv",
    "to txt",
    "to xml",
    "to yaml",
    "to yml",
    "tui bind",
    "tui box",
    "tui button",
    "tui debug",
    "tui label",
    "tui log",
    "tui menu",
    "tui preview",
    "tui progress",
    "tui run",
    "tui search",
    "tui select",
    "tui split",
    "tui tab",
    "tui table",
    "tui textbox",
    "tui tree",
    "update cells",
    "url build-query",
    "url decode",
    "url encode",
    "url join",
    "url parse",
    "url split-query",
    "version check",
    "view blocks",
    "view files",
    "view ir",
    "view source",
    "view span",
];

#[cfg(test)]
mod tests {
    use super::{
        ATTRIBUTES, COMMANDS, EXTERNAL_AFTER_PERCENT, LONGEST_NAME, MULTIWORD_NAMES, is_command,
        is_parser_keyword, name_words, reads_before_assignment, resolves_after_percent,
        row_condition_words, starts_with_parser_keyword, takes_any_argument,
    };
    use crate::test_support::asked;

    fn head_and_next(name: &str) -> (&str, Option<&str>) {
        name.split_once(' ')
            .map_or((name, None), |(head, next)| (head, Some(next)))
    }

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
        for list in [
            ATTRIBUTES,
            COMMANDS,
            EXTERNAL_AFTER_PERCENT,
            MULTIWORD_NAMES,
        ] {
            assert!(list.windows(2).all(|pair| pair[0] < pair[1]), "{list:?}");
        }
    }

    #[test]
    fn every_name_after_percent_is_command() {
        assert!(EXTERNAL_AFTER_PERCENT.iter().all(|name| is_command(name)));
    }

    #[test]
    fn commands_are_what_nu_has() {
        let Some(names) = asked(COMMANDS_OF_NU) else {
            return;
        };
        assert_eq!(without_windows_only(COMMANDS), names);
    }

    /// The tests run nu on unix.
    fn without_windows_only<'a>(names: &[&'a str]) -> Vec<&'a str> {
        names
            .iter()
            .copied()
            .filter(|name| !WINDOWS_ONLY.contains(name))
            .collect()
    }

    const WINDOWS_ONLY: &[&str] = &["registry", "registry query"];

    #[test]
    fn multiword_names_are_what_nu_has() {
        let Some(names) = asked(NAMES_OF_NU) else {
            return;
        };
        let multiword: Vec<&str> = names
            .iter()
            .map(String::as_str)
            .filter(|name| name.contains(' '))
            .collect();
        assert_eq!(without_windows_only(MULTIWORD_NAMES), multiword);
    }

    #[test]
    fn name_words_takes_longest_name() {
        for (words, count) in [
            (&["str", "replace", "a"][..], 2),
            (&["attr", "complete", "external", "x"], 3),
            (&["str"], 1),
            (&["ls", "foo", "bar"], 1),
            (&["http", "get", "url"], 2),
            (&[], 1),
        ] {
            assert_eq!(name_words(words), count, "{words:?}");
        }
    }

    #[test]
    fn longest_name_is_that_of_most_words() {
        let most = MULTIWORD_NAMES
            .iter()
            .map(|name| name.split(' ').count())
            .max();
        assert_eq!(most, Some(LONGEST_NAME));
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
    fn row_condition_commands_are_what_nu_declares() {
        let (Some(names), Some(declared)) = (asked(NAMES_OF_NU), asked(ROW_CONDITIONS_OF_NU))
        else {
            return;
        };
        let with_condition: Vec<&str> = names
            .iter()
            .map(String::as_str)
            .filter(|name| {
                let (head, next) = head_and_next(name);
                row_condition_words(head, next).is_some()
            })
            .collect();
        assert_eq!(with_condition, declared);
    }

    #[test]
    fn any_argument_keywords_are_what_nu_declares() {
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
    fn keywords_read_before_assignment_are_what_nu_dispatches_first() {
        let (Some(names), Some(before)) = (asked(NAMES_OF_NU), asked(BEFORE_ASSIGNMENT_OF_NU))
        else {
            return;
        };
        let read_before: Vec<&str> = names
            .iter()
            .map(String::as_str)
            .filter(|name| {
                let (head, next) = head_and_next(name);
                reads_before_assignment(head, next)
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
            let (head, next) = head_and_next(keyword);
            assert!(starts_with_parser_keyword(head, next), "{keyword:?}");
        }
    }
}
