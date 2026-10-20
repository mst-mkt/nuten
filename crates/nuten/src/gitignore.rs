use std::{
    env, fs, io, iter,
    path::{Path, PathBuf},
};

use gix_ignore::{
    glob::{
        pattern::Case,
        search::pattern::{List, Mapping},
    },
    search::{Ignore, pattern_matching_relative_path},
};

pub(crate) type IgnorePatterns = List<Ignore>;

#[derive(Debug)]
pub(crate) struct UnreadFile {
    pub(crate) path: PathBuf,
    pub(crate) error: io::Error,
}

#[derive(Debug)]
pub(crate) struct DirIgnores {
    prefix_len: usize,
    is_repo: bool,
    dot_ignore: Option<IgnorePatterns>,
    gitignore: Option<IgnorePatterns>,
    exclude: Option<IgnorePatterns>,
}

impl DirIgnores {
    /// Takes `holds`, so that an absent file costs no system call.
    /// Takes `reads_gitignore`, so that a gitignore that applies nowhere stops no walk when unread.
    pub(crate) fn read(
        dir: &Path,
        prefix_len: usize,
        is_repo: bool,
        reads_gitignore: bool,
        holds: impl Fn(&str) -> bool,
    ) -> Result<DirIgnores, Vec<UnreadFile>> {
        let read_held = |name: &str| {
            if holds(name) {
                read(&dir.join(name))
            } else {
                Ok(None)
            }
        };
        let dot_ignore = read_held(".ignore");
        let (gitignore, exclude) = if reads_gitignore {
            let gitignore = read_held(".gitignore");
            let exclude = if holds(".git") {
                git_dir(&dir.join(".git")).and_then(|git_dir| read(&git_dir.join("info/exclude")))
            } else {
                Ok(None)
            };
            (gitignore, exclude)
        } else {
            (Ok(None), Ok(None))
        };
        match (dot_ignore, gitignore, exclude) {
            (Ok(dot_ignore), Ok(gitignore), Ok(exclude)) => Ok(DirIgnores {
                prefix_len,
                is_repo,
                dot_ignore,
                gitignore,
                exclude,
            }),
            (dot_ignore, gitignore, exclude) => {
                Err([dot_ignore.err(), gitignore.err(), exclude.err()]
                    .into_iter()
                    .flatten()
                    .collect())
            },
        }
    }

    pub(crate) fn is_repo(&self) -> bool {
        self.is_repo
    }
}

pub(crate) fn is_repo(holds: impl Fn(&str) -> bool) -> bool {
    holds(".git") || holds(".jj")
}

/// `levels` run from the top of the file system down to the directory holding `path`.
pub(crate) fn ignores(
    levels: &[&DirIgnores],
    global_excludes: Option<&IgnorePatterns>,
    path: &[u8],
    is_dir: bool,
) -> bool {
    let verdict_at = |patterns: Option<&IgnorePatterns>, prefix_len: usize| {
        patterns.and_then(|patterns| verdict(patterns, &path[prefix_len..], is_dir))
    };
    let in_repo = levels
        .iter()
        .rposition(|level| level.is_repo)
        .map(|repo| &levels[repo..]);
    let verdict_in_repo = |patterns: fn(&DirIgnores) -> Option<&IgnorePatterns>| {
        in_repo?
            .iter()
            .rev()
            .find_map(|level| verdict_at(patterns(level), level.prefix_len))
    };
    levels
        .iter()
        .rev()
        .find_map(|level| verdict_at(level.dot_ignore.as_ref(), level.prefix_len))
        .or_else(|| verdict_in_repo(|level| level.gitignore.as_ref()))
        .or_else(|| verdict_in_repo(|level| level.exclude.as_ref()))
        .or_else(|| verdict_at(global_excludes, in_repo?.first()?.prefix_len))
        .unwrap_or(false)
}

fn verdict(patterns: &IgnorePatterns, path: &[u8], is_dir: bool) -> Option<bool> {
    let basename = path.iter().rposition(|&byte| byte == b'/').map(|at| at + 1);
    pattern_matching_relative_path(
        patterns,
        path.into(),
        basename,
        Some(is_dir),
        Case::Sensitive,
    )
    .map(|found| !found.pattern.is_negative())
}

fn read(path: &Path) -> Result<Option<IgnorePatterns>, UnreadFile> {
    match fs::read(path) {
        Ok(bytes) => Ok(Some(parse(&bytes, path))),
        Err(error) if is_absent(path, &error) => Ok(None),
        Err(error) => Err(UnreadFile {
            path: path.to_path_buf(),
            error,
        }),
    }
}

/// A directory in place of the file ignores nothing too, as in git.
fn is_absent(path: &Path, error: &io::Error) -> bool {
    matches!(
        error.kind(),
        io::ErrorKind::NotFound | io::ErrorKind::NotADirectory | io::ErrorKind::IsADirectory
    ) || path.is_dir()
}

fn parse(bytes: &[u8], source: &Path) -> IgnorePatterns {
    List {
        // Not reading `$` as a mark of a precious file, since git does not.
        patterns: gix_ignore::parse(bytes, false)
            .map(|(pattern, line, kind)| Mapping {
                pattern,
                value: kind,
                sequence_number: line,
            })
            .collect(),
        source: Some(source.to_path_buf()),
        base: None,
    }
}

/// A `.git` file names the git directory of a worktree or submodule.
fn git_dir(dot_git: &Path) -> Result<PathBuf, UnreadFile> {
    if !dot_git.is_file() {
        return Ok(dot_git.to_path_buf());
    }
    let text = fs::read_to_string(dot_git).map_err(|error| UnreadFile {
        path: dot_git.to_path_buf(),
        error,
    })?;
    let Some(named) = text
        .lines()
        .next()
        .and_then(|line| line.strip_prefix("gitdir:"))
    else {
        return Ok(dot_git.to_path_buf());
    };
    let git_dir = dot_git.parent().unwrap_or(Path::new("")).join(named.trim());
    let commondir = git_dir.join("commondir");
    match fs::read_to_string(&commondir) {
        Ok(common) => Ok(git_dir.join(common.trim_end_matches(['\n', '\r']))),
        Err(error) if is_absent(&commondir, &error) => Ok(git_dir),
        Err(error) => Err(UnreadFile {
            path: commondir,
            error,
        }),
    }
}

/// Not the config of a repository, nor files included from it, which ripgrep does not read either.
pub(crate) fn global_excludes() -> Result<Option<IgnorePatterns>, UnreadFile> {
    match excludes_file() {
        Some(path) => read(&path),
        None => Ok(None),
    }
}

fn excludes_file() -> Option<PathBuf> {
    config_files()
        .iter()
        .rev()
        .filter_map(|path| fs::read(path).ok())
        .find_map(|config| excludes_file_in(&String::from_utf8_lossy(&config)))
        .map(|value| expand_home(&value))
        .or_else(|| Some(config_home()?.join("git/ignore")))
}

/// In the order git reads them, so that the last one wins.
fn config_files() -> Vec<PathBuf> {
    let system =
        non_empty_var("GIT_CONFIG_SYSTEM").unwrap_or_else(|| PathBuf::from("/etc/gitconfig"));
    let global = match non_empty_var("GIT_CONFIG_GLOBAL") {
        Some(path) => vec![path],
        None => [
            config_home().map(|home| home.join("git/config")),
            env::home_dir().map(|home| home.join(".gitconfig")),
        ]
        .into_iter()
        .flatten()
        .collect(),
    };
    let skips_system = non_empty_var("GIT_CONFIG_NOSYSTEM").is_some();
    (!skips_system)
        .then_some(system)
        .into_iter()
        .chain(global)
        .collect()
}

fn non_empty_var(name: &str) -> Option<PathBuf> {
    env::var_os(name)
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
}

fn config_home() -> Option<PathBuf> {
    non_empty_var("XDG_CONFIG_HOME").or_else(|| Some(env::home_dir()?.join(".config")))
}

fn expand_home(value: &str) -> PathBuf {
    let below = if value == "~" {
        Some("")
    } else {
        value.strip_prefix("~/")
    };
    below
        .and_then(|below| Some(env::home_dir()?.join(below)))
        .unwrap_or_else(|| PathBuf::from(value))
}

fn excludes_file_in(config: &str) -> Option<String> {
    joined_lines(config)
        .scan(false, |in_core, line| {
            let line = line.trim_start();
            let entry = match line.strip_prefix('[').and_then(|rest| rest.split_once(']')) {
                Some((section, rest)) => {
                    *in_core = section.trim().eq_ignore_ascii_case("core");
                    rest
                },
                None => line,
            };
            Some(in_core.then(|| entry.to_owned()))
        })
        .flatten()
        .filter_map(|entry| {
            let (key, value) = entry.split_once('=')?;
            key.trim()
                .eq_ignore_ascii_case("excludesfile")
                .then(|| config_value(value))
        })
        .last()
}

fn joined_lines(config: &str) -> impl Iterator<Item = String> + '_ {
    let mut lines = config.lines();
    iter::from_fn(move || {
        let first = lines.next()?;
        let mut joined = String::from(first);
        while continues(&joined)
            && let Some(next) = lines.next()
        {
            joined.pop();
            joined.push_str(next);
        }
        Some(joined)
    })
}

/// Not after a comment, which git reads to the end of the line.
fn continues(line: &str) -> bool {
    let mut chars = line.chars();
    let mut quoted = false;
    while let Some(char) = chars.next() {
        match char {
            '"' => quoted = !quoted,
            '#' | ';' if !quoted => return false,
            '\\' if chars.next().is_none() => return true,
            _ => {},
        }
    }
    false
}

fn config_value(raw: &str) -> String {
    let mut chars = raw.trim_start().chars();
    let mut quoted = false;
    let unquoted: Vec<(char, bool)> = iter::from_fn(|| {
        loop {
            match chars.next()? {
                '"' => quoted = !quoted,
                '#' | ';' if !quoted => return None,
                '\\' => return Some((unescaped(chars.next()?), false)),
                char => return Some((char, !quoted && char.is_whitespace())),
            }
        }
    })
    .collect();
    let kept = unquoted
        .iter()
        .rposition(|&(_, droppable)| !droppable)
        .map_or(0, |at| at + 1);
    unquoted[..kept].iter().map(|&(char, _)| char).collect()
}

fn unescaped(char: char) -> char {
    match char {
        'n' => '\n',
        't' => '\t',
        'b' => '\u{8}',
        other => other,
    }
}

#[cfg(test)]
mod tests {
    use super::excludes_file_in;

    #[test]
    fn excludes_file_is_read_from_core_section_without_subsection_only() {
        let config = concat!(
            "[user]\nexcludesFile = /a\n",
            "[core]\n\texcludesfile = /b\n",
            "[core \"x\"]\nexcludesFile = /c\n",
        );
        assert_eq!(excludes_file_in(config).as_deref(), Some("/b"));
    }

    #[test]
    fn last_excludes_file_wins() {
        let config = "[core]\nexcludesFile = /a\n[core]\nExcludesFile = /b\n";
        assert_eq!(excludes_file_in(config).as_deref(), Some("/b"));
    }

    #[test]
    fn excludes_file_value_is_unquoted_unescaped_and_trimmed() {
        for (value, expected) in [
            ("\"/a b\" # comment", "/a b"),
            ("/a ; comment", "/a"),
            ("\"/a#b\"", "/a#b"),
            (r#""/a\"b""#, "/a\"b"),
            ("/a  ", "/a"),
        ] {
            let config = format!("[core]\nexcludesFile = {value}\n");
            assert_eq!(
                excludes_file_in(&config).as_deref(),
                Some(expected),
                "{value}"
            );
        }
    }

    #[test]
    fn entry_after_section_header_on_same_line_is_read() {
        assert_eq!(
            excludes_file_in("[core] excludesFile = /a\n").as_deref(),
            Some("/a")
        );
    }

    #[test]
    fn excludes_file_value_continues_onto_next_line_after_unescaped_backslash() {
        assert_eq!(
            excludes_file_in("[core]\nexcludesFile = /a\\\n b\n").as_deref(),
            Some("/a b")
        );
        assert_eq!(
            excludes_file_in("[core]\nexcludesFile = /a\\\nb\n").as_deref(),
            Some("/ab")
        );
    }

    #[test]
    fn excludes_file_value_ends_at_line_ending_in_escaped_backslash() {
        assert_eq!(
            excludes_file_in("[core]\nexcludesFile = /a\\\\\nb = c\n").as_deref(),
            Some("/a\\")
        );
    }

    #[test]
    fn comment_ending_in_backslash_ends_at_end_of_its_line() {
        for config in [
            "[core]\n# C:\\tools\\\n excludesFile = /x\n",
            "[user]\n; note \\\n[core]\nexcludesFile = /x\n",
            "[core]\nexcludesFile = /x ; note \\\n",
        ] {
            assert_eq!(excludes_file_in(config).as_deref(), Some("/x"), "{config}");
        }
    }

    #[test]
    fn excludes_file_value_continues_after_comment_character_in_quotes() {
        for (config, value) in [
            ("[core]\nexcludesFile = \"/a;b\\\nc\"\n", "/a;bc"),
            ("[core]\nexcludesFile = \"/a#b\\\nc\"\n", "/a#bc"),
        ] {
            assert_eq!(excludes_file_in(config).as_deref(), Some(value), "{config}");
        }
    }
}
