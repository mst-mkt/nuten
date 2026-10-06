use std::{
    env,
    path::{Path, PathBuf},
};

use serde::Serialize;

use super::absolute_as_written;
use crate::{diff::Difference, problem::Problem};

const FORMAT_CHECK: &str = "nuten/format";

const PROBLEM_CHECK: &str = "nuten/problem";

pub(crate) fn report(issues: impl Iterator<Item = String>) -> String {
    let issues: Vec<_> = issues.collect();
    format!("[{}]\n", issues.join(","))
}

/// Its fingerprint reads the old text, not the lines, so that it stays while other lines move.
pub(crate) fn file_issue(path: &Path, difference: &Difference<'_>) -> String {
    let lines = difference.changed_span();
    let old_text = difference
        .old_lines
        .get(lines.start() - 1..*lines.end())
        .unwrap_or_default()
        .concat();
    let path = path_in_project(path, project_dir().as_deref());
    let path_text = path_text(&path);
    issue(&Issue {
        description: "Would be reformatted",
        check_name: FORMAT_CHECK,
        fingerprint: fingerprint(&[
            FORMAT_CHECK.as_bytes(),
            &path_bytes(&path),
            old_text.as_bytes(),
        ]),
        severity: "minor",
        location: Location {
            path: &path_text,
            lines: Lines {
                begin: *lines.start(),
                end: *lines.end(),
            },
        },
    })
}

/// None for a problem of no file, which an issue cannot hold.
pub(crate) fn problem_issue(problem: &Problem) -> Option<String> {
    let path = path_in_project(problem.path.as_deref()?, project_dir().as_deref());
    let path_text = path_text(&path);
    let line = problem.position.map_or(1, |position| position.line);
    let column = problem
        .position
        .and_then(|position| position.column)
        .map_or(String::new(), |column| column.to_string());
    Some(issue(&Issue {
        description: &problem.message,
        check_name: PROBLEM_CHECK,
        // With the position, since a problem holds no text of its own to follow as lines move.
        fingerprint: fingerprint(&[
            PROBLEM_CHECK.as_bytes(),
            &path_bytes(&path),
            line.to_string().as_bytes(),
            column.as_bytes(),
            problem.message.as_bytes(),
        ]),
        severity: "major",
        location: Location {
            path: &path_text,
            lines: Lines {
                begin: line,
                end: line,
            },
        },
    }))
}

#[derive(Serialize)]
struct Issue<'a> {
    description: &'a str,
    check_name: &'a str,
    fingerprint: String,
    severity: &'a str,
    location: Location<'a>,
}

#[derive(Serialize)]
struct Location<'a> {
    path: &'a str,
    lines: Lines,
}

#[derive(Serialize)]
struct Lines {
    begin: usize,
    end: usize,
}

fn issue(issue: &Issue<'_>) -> String {
    serde_json::to_string(issue).expect("an issue holds only strings and numbers")
}

/// The directory GitLab CI cloned the repository into, as other formatters read it.
fn project_dir() -> Option<PathBuf> {
    env::var_os("CI_PROJECT_DIR")
        .filter(|dir| !dir.is_empty())
        .map(PathBuf::from)
}

/// Relative, since GitLab reads a path from the root of the repository.
/// The current directory stands for the root outside GitLab CI, as other formatters take it.
/// Absolute outside the root, which GitLab cannot place but whoever reads the report can find.
fn path_in_project(path: &Path, project_dir: Option<&Path>) -> PathBuf {
    let path = absolute_as_written(path).unwrap_or_else(|_| path.to_path_buf());
    let root = project_dir
        .map_or_else(|| absolute_as_written(Path::new(".")), absolute_as_written)
        .ok();
    root.as_deref()
        .and_then(|root| path.strip_prefix(root).ok())
        .map_or_else(|| path.clone(), Path::to_path_buf)
}

/// Joined with `/`, so that runners of every system agree on a fingerprint.
fn path_bytes(path: &Path) -> Vec<u8> {
    let names: Vec<_> = path
        .components()
        .map(|name| name.as_os_str().as_encoded_bytes())
        .collect();
    names.join(&b'/')
}

fn path_text(path: &Path) -> String {
    if path.is_absolute() {
        return path.display().to_string();
    }
    // Lossy for names that are not UTF-8, as the list of `--list-different` is.
    let names: Vec<_> = path
        .components()
        .map(|name| name.as_os_str().to_string_lossy())
        .collect();
    names.join("/")
}

/// FNV-1a, not `DefaultHasher`, whose values may change between versions of Rust.
/// Each part ends with a NUL, so that no two lists of parts read as one.
/// Of bytes, not text, so that two names alike once made lossy stay apart.
fn fingerprint(parts: &[&[u8]]) -> String {
    let hash = parts
        .iter()
        .flat_map(|part| part.iter().copied().chain([0]))
        .fold(0xcbf2_9ce4_8422_2325_u64, |hash, byte| {
            (hash ^ u64::from(byte)).wrapping_mul(0x0000_0100_0000_01b3)
        });
    format!("{hash:016x}")
}

#[cfg(test)]
mod tests {
    use std::{env, path::Path};

    use serde_json::{Value, json};

    use super::{fingerprint, path_bytes, path_in_project, path_text, problem_issue, report};
    use crate::problem::{Position, Problem};

    #[test]
    fn report_is_empty_list_without_issues() {
        assert_eq!(report([].into_iter()), "[]\n");
    }

    #[test]
    fn report_lists_each_issue() {
        let issues = ["{}".to_string(), "{}".to_string()];
        assert_eq!(report(issues.into_iter()), "[{},{}]\n");
    }

    #[test]
    fn problem_issue_fingerprint_tells_apart_path_position_and_message() {
        let position = |line, column| Position {
            line,
            column: Some(column),
        };
        let fingerprint = |path: &str, position: Position, message: &str| {
            let problem = Problem::at(Path::new(path), position, message);
            let issue: Value = serde_json::from_str(&problem_issue(&problem).unwrap()).unwrap();
            issue["fingerprint"].clone()
        };
        let base = fingerprint("a.nu", position(2, 3), "e");
        assert_ne!(fingerprint("b.nu", position(2, 3), "e"), base);
        assert_ne!(fingerprint("a.nu", position(4, 3), "e"), base);
        assert_ne!(fingerprint("a.nu", position(2, 5), "e"), base);
        assert_ne!(fingerprint("a.nu", position(2, 3), "f"), base);
    }

    #[test]
    fn problem_issue_holds_line_of_problem_and_first_line_without_one() {
        let path = Path::new("a.nu");
        let position = Position {
            line: 2,
            column: Some(3),
        };
        let lines = |problem: &Problem| {
            let issue: Value = serde_json::from_str(&problem_issue(problem).unwrap()).unwrap();
            issue["location"]["lines"].clone()
        };
        assert_eq!(
            lines(&Problem::at(path, position, "e")),
            json!({ "begin": 2, "end": 2 })
        );
        assert_eq!(
            lines(&Problem::in_file(path, "e")),
            json!({ "begin": 1, "end": 1 })
        );
    }

    #[test]
    fn problem_issue_writes_keys_in_order_of_gitlab_documentation() {
        let issue = problem_issue(&Problem::in_file(Path::new("a.nu"), "e")).unwrap();
        assert!(
            issue.starts_with(r#"{"description":"e","check_name":"nuten/problem","fingerprint":"#),
            "{issue}"
        );
        assert!(
            issue.ends_with(
                r#","severity":"major","location":{"path":"a.nu","lines":{"begin":1,"end":1}}}"#
            ),
            "{issue}"
        );
    }

    #[test]
    fn problem_issue_is_none_for_problem_of_no_file() {
        assert_eq!(problem_issue(&Problem::new("e")), None);
    }

    #[test]
    fn path_in_project_is_relative_to_project_dir_with_slashes() {
        let root = env::current_dir().unwrap();
        let path = root.join("a").join("b.nu");
        assert_eq!(path_text(&path_in_project(&path, Some(&root))), "a/b.nu");
        assert_eq!(
            path_text(&path_in_project(Path::new("a/../b.nu"), None)),
            "b.nu"
        );
    }

    #[test]
    fn path_in_project_stays_absolute_outside_project_dir() {
        let root = env::current_dir().unwrap().join("project");
        let path = env::current_dir().unwrap().join("b.nu");
        assert_eq!(
            path_text(&path_in_project(&path, Some(&root))),
            path.display().to_string()
        );
    }

    #[test]
    fn path_bytes_join_names_with_slash_on_every_system() {
        assert_eq!(path_bytes(&Path::new("a").join("b.nu")), b"a/b.nu");
    }

    #[test]
    fn fingerprint_is_fnv1a_of_parts_each_ended_with_nul() {
        assert_eq!(fingerprint(&[]), "cbf29ce484222325");
        assert_eq!(fingerprint(&[b"a"]), "089be207b544f1e4");
        assert_ne!(fingerprint(&[b"ab", b"c"]), fingerprint(&[b"a", b"bc"]));
    }
}
