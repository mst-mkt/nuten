use std::rc::Rc;

/// What the parser of every level of brackets knows of the names in the file.
#[derive(Clone)]
pub(crate) struct Names<'a> {
    file: Rc<FileNames<'a>>,
}

/// What the file names, whatever block a name stands in.
struct FileNames<'a> {
    /// The names the file aliases, sorted so that `is_aliased` can bisect them.
    aliases: Box<[&'a str]>,
}

impl<'a> Names<'a> {
    pub(crate) fn of(src: &'a str) -> Names<'a> {
        Names {
            file: Rc::new(FileNames {
                aliases: alias_names(src),
            }),
        }
    }

    pub(crate) fn is_aliased(&self, name: &str) -> bool {
        self.file.aliases.binary_search(&name).is_ok()
    }
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

/// What nu may start a statement after, so that an `alias` first in one is a head.
fn breaks_statement(char: char) -> bool {
    matches!(char, '\n' | '\r' | ';' | '|' | '{' | '}' | '(' | ')')
}

const NAME_QUOTES: [char; 3] = ['"', '\'', '`'];

fn alias_name(statement: &str) -> Option<&str> {
    let mut words = statement.split_ascii_whitespace();
    let first = words.next()?;
    let head = if first == "export" {
        words.next()?
    } else {
        first
    };
    if head != "alias" {
        return None;
    }
    Some(words.next()?.trim_matches(NAME_QUOTES))
}

#[cfg(test)]
mod tests {
    use super::alias_names;

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
