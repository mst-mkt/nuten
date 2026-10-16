use std::{fmt, path::Path, sync::Arc};

use globset::{GlobBuilder, GlobSet, GlobSetBuilder};

#[derive(Debug, Clone)]
pub struct Patterns {
    written: Arc<[String]>,
    anchored: Arc<GlobSet>,
    names: Arc<GlobSet>,
}

impl PartialEq for Patterns {
    fn eq(&self, other: &Self) -> bool {
        self.written == other.written
    }
}

impl Eq for Patterns {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PatternProblem {
    Empty,
    Negated,
    ParentDir,
    NoName,
    Invalid(String),
}

impl fmt::Display for PatternProblem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PatternProblem::Empty => f.write_str("is empty"),
            PatternProblem::Negated => f.write_str(
                "starts with `!`, which negates nothing; `include` picks files, and `exclude` \
                 leaves them out",
            ),
            PatternProblem::ParentDir => f.write_str(
                "has `..`, though patterns match only below the directory they are relative to",
            ),
            PatternProblem::NoName => {
                f.write_str("has `.` or nothing where a name goes, and so matches no file")
            },
            PatternProblem::Invalid(problem) => write!(f, "is invalid: {problem}"),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Place<'a> {
    pub relative: Option<&'a Path>,
    pub name: &'a Path,
}

impl Place<'_> {
    /// Both are canonical.
    pub fn of<'a>(file: &'a Path, root: Option<&Path>) -> Place<'a> {
        let relative = root.and_then(|root| file.strip_prefix(root).ok());
        let name = file.file_name().map_or(file, Path::new);
        Place {
            relative,
            name,
        }
    }
}

impl Patterns {
    pub fn new<'a>(
        written: impl IntoIterator<Item = &'a str>,
    ) -> Result<Patterns, Vec<(usize, PatternProblem)>> {
        let written: Vec<String> = written.into_iter().map(str::to_string).collect();
        let parsed: Vec<_> = written.iter().map(|pattern| parse(pattern)).collect();
        let problems: Vec<_> = parsed
            .iter()
            .enumerate()
            .filter_map(|(index, parsed)| Some((index, parsed.as_ref().err()?.clone())))
            .collect();
        if !problems.is_empty() {
            return Err(problems);
        }
        let (anchored, names): (Vec<_>, Vec<_>) = parsed
            .into_iter()
            .flatten()
            .partition(|parsed| parsed.anchored);
        Ok(Patterns {
            written: written.into(),
            anchored: set(anchored),
            names: set(names),
        })
    }

    pub fn is_empty(&self) -> bool {
        self.written.is_empty()
    }

    pub fn has_anchored(&self) -> bool {
        !self.anchored.is_empty()
    }

    pub fn matches_file(&self, place: Place<'_>) -> bool {
        self.names.is_match(place.name)
            || place
                .relative
                .is_some_and(|relative| self.anchored.is_match(relative))
    }

    pub fn matches_file_or_ancestor(&self, place: Place<'_>) -> bool {
        let Some(relative) = place.relative else {
            return self.names.is_match(place.name);
        };
        relative
            .ancestors()
            .filter(|path| !path.as_os_str().is_empty())
            .any(|path| {
                self.anchored.is_match(path)
                    || path
                        .file_name()
                        .is_some_and(|name| self.names.is_match(name))
            })
    }
}

struct Parsed {
    glob: globset::Glob,
    anchored: bool,
}

fn parse(pattern: &str) -> Result<Parsed, PatternProblem> {
    if pattern.starts_with('!') {
        return Err(PatternProblem::Negated);
    }
    // A `/` at the end names a directory in gitignore, and every name here may be one.
    let trimmed = pattern.trim_end_matches('/');
    let anchored = trimmed.contains('/');
    let relative = trimmed
        .strip_prefix("./")
        .or_else(|| trimmed.strip_prefix('/'))
        .unwrap_or(trimmed);
    if relative.is_empty() {
        return Err(PatternProblem::Empty);
    }
    if relative.split('/').any(is_parent_dir) {
        return Err(PatternProblem::ParentDir);
    }
    if relative
        .split('/')
        .any(|part| part.is_empty() || part == ".")
    {
        return Err(PatternProblem::NoName);
    }
    // Not escaping as the platform does, so that one config file reads the same on each.
    let glob = GlobBuilder::new(relative)
        .literal_separator(true)
        .backslash_escape(true)
        .build()
        .map_err(|error| PatternProblem::Invalid(error.kind().to_string()))?;
    Ok(Parsed {
        glob,
        anchored,
    })
}

/// An alternative of a `{}` group counts too, since a glob may take any of them.
fn is_parent_dir(part: &str) -> bool {
    part.match_indices("..").any(|(start, _)| {
        let before = part[..start].chars().next_back();
        let after = part[start + 2..].chars().next();
        before.is_none_or(|c| matches!(c, '{' | ','))
            && after.is_none_or(|c| matches!(c, ',' | '}'))
    })
}

fn set(parsed: Vec<Parsed>) -> Arc<GlobSet> {
    let mut builder = GlobSetBuilder::new();
    for parsed in parsed {
        builder.add(parsed.glob);
    }
    Arc::new(builder.build().expect("globs that each build make a set"))
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::{PatternProblem, Patterns, Place};

    const ROOT: &str = "/r";

    fn place(file: &str) -> Place<'_> {
        Place::of(Path::new(file), Some(Path::new(ROOT)))
    }

    fn matching_files<'a>(pattern: &str, files: &[&'a str]) -> Vec<&'a str> {
        let patterns = Patterns::new([pattern]).unwrap();
        files
            .iter()
            .copied()
            .filter(|file| patterns.matches_file(place(file)))
            .collect()
    }

    fn matching_files_or_ancestors<'a>(pattern: &str, files: &[&'a str]) -> Vec<&'a str> {
        let patterns = Patterns::new([pattern]).unwrap();
        files
            .iter()
            .copied()
            .filter(|file| patterns.matches_file_or_ancestor(place(file)))
            .collect()
    }

    #[test]
    fn pattern_without_slash_matches_file_name_at_any_depth() {
        let files = ["/r/a.nu", "/r/d/a.nu", "/r/a.nu/b", "/r/b.nu"];
        assert_eq!(matching_files("a.nu", &files), ["/r/a.nu", "/r/d/a.nu"]);
        assert_eq!(
            matching_files("*.nu", &files),
            ["/r/a.nu", "/r/d/a.nu", "/r/b.nu"]
        );
    }

    #[test]
    fn pattern_with_slash_matches_path_from_root() {
        let files = ["/r/d/a.nu", "/r/e/d/a.nu", "/r/d/e/a.nu"];
        for pattern in ["d/a.nu", "/d/a.nu", "./d/a.nu"] {
            assert_eq!(matching_files(pattern, &files), ["/r/d/a.nu"], "{pattern}");
        }
        assert_eq!(matching_files("d/*", &files), ["/r/d/a.nu"]);
        assert_eq!(matching_files("d/**", &files), ["/r/d/a.nu", "/r/d/e/a.nu"]);
        assert_eq!(
            matching_files("**/d/a.nu", &files),
            ["/r/d/a.nu", "/r/e/d/a.nu"]
        );
    }

    #[test]
    fn leading_slash_anchors_name_to_root() {
        let files = ["/r/a.nu", "/r/d/a.nu"];
        assert_eq!(matching_files("/a.nu", &files), ["/r/a.nu"]);
    }

    #[test]
    fn pattern_with_trailing_slash_matches_as_without_it() {
        let files = ["/r/d/a.nu", "/r/e/d/a.nu", "/r/e/a.nu"];
        assert_eq!(
            matching_files_or_ancestors("d/", &files),
            ["/r/d/a.nu", "/r/e/d/a.nu"]
        );
        assert_eq!(matching_files_or_ancestors("/d/", &files), ["/r/d/a.nu"]);
    }

    #[test]
    fn star_stays_within_one_name() {
        assert_eq!(
            matching_files("d/*.nu", &["/r/d/a.nu", "/r/d/e/a.nu"]),
            ["/r/d/a.nu"]
        );
    }

    #[test]
    fn braces_and_classes_match_either() {
        let files = ["/r/a.nu", "/r/a.nush", "/r/b.nu"];
        assert_eq!(
            matching_files("a.{nu,nush}", &files),
            ["/r/a.nu", "/r/a.nush"]
        );
        assert_eq!(matching_files("[ab].nu", &files), ["/r/a.nu", "/r/b.nu"]);
    }

    #[test]
    fn match_is_case_sensitive() {
        assert_eq!(
            matching_files("a.nu", &["/r/A.nu", "/r/a.NU"]),
            Vec::<&str>::new()
        );
    }

    #[test]
    fn backslash_escapes_on_every_platform() {
        assert_eq!(
            matching_files(r"\*.nu", &["/r/*.nu", "/r/a.nu"]),
            ["/r/*.nu"]
        );
    }

    #[test]
    fn file_matches_not_by_ancestor_alone() {
        assert_eq!(matching_files("d", &["/r/d/a.nu"]), Vec::<&str>::new());
        assert_eq!(matching_files("/d", &["/r/d/a.nu"]), Vec::<&str>::new());
    }

    #[test]
    fn file_or_ancestor_matches_by_any_directory_up_to_root() {
        let files = ["/r/d/a.nu", "/r/e/d/f/a.nu", "/r/e/a.nu", "/d/r/a.nu"];
        assert_eq!(
            matching_files_or_ancestors("d", &files),
            ["/r/d/a.nu", "/r/e/d/f/a.nu"]
        );
        assert_eq!(
            matching_files_or_ancestors("e/d", &files),
            ["/r/e/d/f/a.nu"]
        );
    }

    #[test]
    fn file_outside_root_matches_only_by_its_name() {
        let patterns = Patterns::new(["a.nu", "d", "d/a.nu"]).unwrap();
        let outside = Place::of(Path::new("/o/d/a.nu"), Some(Path::new(ROOT)));
        assert!(patterns.matches_file(outside));
        let outside = Place::of(Path::new("/o/d/b.nu"), Some(Path::new(ROOT)));
        assert!(!patterns.matches_file_or_ancestor(outside));
        let rootless = Place::of(Path::new("/r/d/a.nu"), None);
        let anchored = Patterns::new(["d/a.nu"]).unwrap();
        assert!(!anchored.matches_file(rootless));
    }

    #[cfg(windows)]
    #[test]
    fn pattern_with_slash_matches_path_with_separator_of_platform() {
        let patterns = Patterns::new(["d/a.nu"]).unwrap();
        let place = Place::of(Path::new(r"C:\r\d\a.nu"), Some(Path::new(r"C:\r")));
        assert!(patterns.matches_file(place));
        assert!(patterns.matches_file_or_ancestor(place));
    }

    #[test]
    fn root_itself_is_no_ancestor_patterns_match() {
        let patterns = Patterns::new(["r"]).unwrap();
        assert!(!patterns.matches_file_or_ancestor(place("/r/a.nu")));
    }

    #[test]
    fn no_patterns_match_nothing() {
        let patterns = Patterns::new([]).unwrap();
        assert!(!patterns.matches_file(place("/r/a.nu")));
        assert!(!patterns.matches_file_or_ancestor(place("/r/a.nu")));
        assert!(!patterns.has_anchored());
    }

    #[test]
    fn only_pattern_with_slash_is_anchored() {
        for (pattern, anchored) in [("a", false), ("a/", false), ("/a", true), ("a/b", true)] {
            let patterns = Patterns::new([pattern]).unwrap();
            assert_eq!(patterns.has_anchored(), anchored, "{pattern}");
        }
    }

    #[test]
    fn each_pattern_with_problem_is_reported_with_its_index() {
        let problems =
            Patterns::new(["a", "!a", "", "/", "../a", "a/../b", "a[", "ok/"]).unwrap_err();
        assert_eq!(
            problems,
            [
                (1, PatternProblem::Negated),
                (2, PatternProblem::Empty),
                (3, PatternProblem::Empty),
                (4, PatternProblem::ParentDir),
                (5, PatternProblem::ParentDir),
                (
                    6,
                    PatternProblem::Invalid("unclosed character class; missing ']'".to_string())
                ),
            ]
        );
    }

    #[test]
    fn pattern_with_parent_dir_among_alternatives_is_reported() {
        for pattern in ["a/{b,..}", "{..}/a", "a/{..,b}/c"] {
            assert_eq!(
                Patterns::new([pattern]).unwrap_err(),
                [(0, PatternProblem::ParentDir)],
                "{pattern}"
            );
        }
        assert!(Patterns::new(["a/{b,..c}", "{a,b}..", "a.."]).is_ok());
    }

    #[test]
    fn pattern_with_dot_or_nothing_where_name_goes_is_reported() {
        for pattern in [".", "./", "//a", ".//a", "a//b", "a/./b", "/./", "a/."] {
            assert_eq!(
                Patterns::new([pattern]).unwrap_err(),
                [(0, PatternProblem::NoName)],
                "{pattern}"
            );
        }
        assert!(Patterns::new([".a", "a.", "./.a"]).is_ok());
    }

    #[test]
    fn patterns_equal_by_what_is_written() {
        assert_eq!(Patterns::new(["a"]).unwrap(), Patterns::new(["a"]).unwrap());
        assert_ne!(
            Patterns::new(["a"]).unwrap(),
            Patterns::new(["/a"]).unwrap()
        );
    }
}
