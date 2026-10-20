use std::{
    fs,
    path::{Path, PathBuf},
    process::Output,
};

use crate::support::{
    BLOCK, FORMATTED, TempDir, UNFORMATTED, assert_succeeds, format_in, io_error_line,
    nuten_command, problems, read, stderr, write_unformatted,
};
#[cfg(unix)]
use crate::support::{fifo, format_in_time};

fn reported_paths(output: &Output) -> Vec<String> {
    problems(output)
        .lines()
        .map(|line| {
            let end = line.find(':').expect("a line reports a path");
            line[..end].to_string()
        })
        .collect()
}

#[test]
fn format_formats_nu_and_nuon_files_under_directory() {
    let dir = TempDir::new();
    let nu = write_unformatted(&dir, ["d/a.nu", "d/e/f/b.nu", "d/c.nuon", "d/e/f/d.nuon"]);
    let other = write_unformatted(
        &dir,
        [
            "d/a.txt",
            "d/a.nu.bak",
            "d/nu",
            "d/nuon",
            "d/e/NU.NU",
            "d/e/A.NUON",
            "a.nu",
            "a.nuon",
        ],
    );
    let output = format_in(&dir, &["d"]);
    assert_succeeds(&output);
    for path in &nu {
        assert_eq!(read(path), FORMATTED, "{}", path.display());
    }
    for path in &other {
        assert_eq!(read(path), UNFORMATTED, "{}", path.display());
    }
}

#[test]
fn format_formats_hidden_files_under_directory() {
    let dir = TempDir::new();
    let paths = write_unformatted(&dir, ["d/.config/nushell/config.nu", "d/.a.nu"]);
    let output = format_in(&dir, &["d"]);
    assert_succeeds(&output);
    for path in &paths {
        assert_eq!(read(path), FORMATTED, "{}", path.display());
    }
}

#[test]
fn format_skips_version_control_directories_under_directory() {
    let dir = TempDir::new();
    let skipped = write_unformatted(
        &dir,
        [
            "d/.git/hooks/a.nu",
            "d/.hg/a.nu",
            "d/.svn/a.nu",
            "d/.bzr/a.nu",
            "d/.jj/a.nu",
            "d/e/.git/a.nu",
        ],
    );
    let taken = write_unformatted(&dir, ["d/.github/a.nu", "d/git/a.nu"]);
    let output = format_in(&dir, &["d"]);
    assert_succeeds(&output);
    for path in &skipped {
        assert_eq!(read(path), UNFORMATTED, "{}", path.display());
    }
    for path in &taken {
        assert_eq!(read(path), FORMATTED, "{}", path.display());
    }
}

#[test]
fn format_skips_files_gitignore_ignores_in_git_repository() {
    let dir = TempDir::new();
    dir.dir("d/.git");
    dir.file("d/.gitignore", "gen/\n*.gen.nu\n");
    dir.file("d/e/.gitignore", "local.nu\n");
    let ignored = write_unformatted(&dir, ["d/gen/a.nu", "d/a.gen.nu", "d/e/local.nu"]);
    let taken = write_unformatted(&dir, ["d/a.nu", "d/local.nu"]);
    let output = format_in(&dir, &["d"]);
    assert_succeeds(&output);
    for path in &ignored {
        assert_eq!(read(path), UNFORMATTED, "{}", path.display());
    }
    for path in &taken {
        assert_eq!(read(path), FORMATTED, "{}", path.display());
    }
}

#[test]
fn format_skips_files_gitignore_above_directory_ignores() {
    let dir = TempDir::new();
    dir.dir(".git");
    dir.file(".gitignore", "gen/\n");
    let [ignored, taken] = write_unformatted(&dir, ["d/gen/a.nu", "d/a.nu"]);
    let output = format_in(&dir, &["d"]);
    assert_succeeds(&output);
    assert_eq!(read(&ignored), UNFORMATTED);
    assert_eq!(read(&taken), FORMATTED);
}

#[test]
fn format_takes_no_gitignore_above_git_repository() {
    let dir = TempDir::new();
    dir.file(".gitignore", "gen/\n");
    dir.dir("d/.git");
    let [path] = write_unformatted(&dir, ["d/gen/a.nu"]);
    let output = format_in(&dir, &["d"]);
    assert_succeeds(&output);
    assert_eq!(read(&path), FORMATTED);
}

#[cfg(unix)]
#[test]
fn format_takes_files_of_git_repository_though_it_cannot_read_gitignore_above_it() {
    let dir = TempDir::new();
    if unreadable(&dir, ".gitignore").is_none() {
        return;
    }
    dir.dir("d/.git");
    let [path] = write_unformatted(&dir, ["d/a.nu"]);
    let output = format_in(&dir, &["d"]);
    assert_succeeds(&output);
    assert_eq!(read(&path), FORMATTED);
}

#[cfg(unix)]
#[test]
fn format_takes_files_under_inner_git_repository_though_it_cannot_read_outer_ignore_files() {
    let dir = TempDir::new();
    let (Some(_), Some(_)) = (
        unreadable(&dir, ".gitignore"),
        unreadable(&dir, ".git/info/exclude"),
    ) else {
        return;
    };
    dir.dir("d/.git");
    let [path] = write_unformatted(&dir, ["d/e/a.nu"]);
    let output = format_in(&dir, &["d/e"]);
    assert_succeeds(&output);
    assert_eq!(read(&path), FORMATTED);
}

#[cfg(unix)]
#[test]
fn format_takes_files_of_inner_git_repository_given_though_it_cannot_read_outer_ignore_files() {
    let dir = TempDir::new();
    let (Some(_), Some(_)) = (
        unreadable(&dir, ".gitignore"),
        unreadable(&dir, ".git/info/exclude"),
    ) else {
        return;
    };
    dir.dir("d/.git");
    let [path] = write_unformatted(&dir, ["d/a.nu"]);
    let output = format_in(&dir, &["d"]);
    assert_succeeds(&output);
    assert_eq!(read(&path), FORMATTED);
}

#[cfg(unix)]
#[test]
fn format_takes_files_outside_git_repository_though_it_cannot_read_gitignore_there() {
    let dir = TempDir::new();
    if unreadable(&dir, "d/e/.gitignore").is_none() {
        return;
    }
    let [path] = write_unformatted(&dir, ["d/e/a.nu"]);
    let output = format_in(&dir, &["d"]);
    assert_succeeds(&output);
    assert_eq!(read(&path), FORMATTED);
}

#[test]
fn format_skips_files_gitignore_ignores_when_dot_git_is_file() {
    let dir = TempDir::new();
    dir.file("d/.git", "gitdir: ../elsewhere\n");
    dir.file("d/.gitignore", "gen/\n");
    let [ignored, taken] = write_unformatted(&dir, ["d/gen/a.nu", "d/a.nu"]);
    let output = format_in(&dir, &["d"]);
    assert_succeeds(&output);
    assert_eq!(read(&ignored), UNFORMATTED);
    assert_eq!(read(&taken), FORMATTED);
}

#[test]
fn format_skips_files_git_exclude_ignores() {
    let dir = TempDir::new();
    dir.file("d/.git/info/exclude", "gen/\n");
    let [ignored, taken] = write_unformatted(&dir, ["d/gen/a.nu", "d/a.nu"]);
    let output = format_in(&dir, &["d"]);
    assert_succeeds(&output);
    assert_eq!(read(&ignored), UNFORMATTED);
    assert_eq!(read(&taken), FORMATTED);
}

#[test]
fn format_skips_files_global_excludes_file_of_git_ignores() {
    let dir = TempDir::new();
    dir.file("config/git/ignore", "gen/\n");
    let config_home = dir.path("config");
    dir.dir("d/.git");
    let [ignored, taken] = write_unformatted(&dir, ["d/gen/a.nu", "d/a.nu"]);
    let output = nuten_command()
        .args(["format", "d"])
        .env("XDG_CONFIG_HOME", config_home)
        .current_dir(&dir.0)
        .output()
        .unwrap();
    assert_succeeds(&output);
    assert_eq!(read(&ignored), UNFORMATTED);
    assert_eq!(read(&taken), FORMATTED);
}

#[test]
fn format_takes_no_gitignore_outside_git_repository() {
    let dir = TempDir::new();
    dir.file("d/.gitignore", "a.nu\n");
    let [path] = write_unformatted(&dir, ["d/a.nu"]);
    let output = format_in(&dir, &["d"]);
    assert_succeeds(&output);
    assert_eq!(read(&path), FORMATTED);
}

#[test]
fn format_skips_files_dot_ignore_ignores_outside_git_repository() {
    let dir = TempDir::new();
    dir.file("d/.ignore", "a.nu\n");
    let [ignored, taken] = write_unformatted(&dir, ["d/a.nu", "d/b.nu"]);
    let output = format_in(&dir, &["d"]);
    assert_succeeds(&output);
    assert_eq!(read(&ignored), UNFORMATTED);
    assert_eq!(read(&taken), FORMATTED);
}

#[test]
fn format_skips_files_gitignore_ignores_in_jujutsu_repository() {
    let dir = TempDir::new();
    dir.dir("d/.jj");
    dir.file("d/.gitignore", "a.nu\n");
    let [ignored, taken] = write_unformatted(&dir, ["d/a.nu", "d/b.nu"]);
    let output = format_in(&dir, &["d"]);
    assert_succeeds(&output);
    assert_eq!(read(&ignored), UNFORMATTED);
    assert_eq!(read(&taken), FORMATTED);
}

#[test]
fn format_takes_no_gitignore_of_repository_enclosing_another() {
    let dir = TempDir::new();
    dir.dir("d/.git");
    dir.file("d/.gitignore", "a.nu\n");
    dir.dir("d/e/.git");
    let [path] = write_unformatted(&dir, ["d/e/a.nu"]);
    let output = format_in(&dir, &["d/e"]);
    assert_succeeds(&output);
    assert_eq!(read(&path), FORMATTED);
}

#[test]
fn format_skips_files_git_exclude_of_main_worktree_ignores_in_linked_worktree() {
    let dir = TempDir::new();
    dir.file("main/.git/info/exclude", "a.nu\n");
    dir.file("main/.git/worktrees/w/commondir", "../..\n");
    dir.file("w/.git", "gitdir: ../main/.git/worktrees/w\n");
    let [ignored, taken] = write_unformatted(&dir, ["w/a.nu", "w/b.nu"]);
    let output = format_in(&dir, &["w"]);
    assert_succeeds(&output);
    assert_eq!(read(&ignored), UNFORMATTED);
    assert_eq!(read(&taken), FORMATTED);
}

#[test]
fn format_reads_directory_in_place_of_gitignore_as_ignoring_nothing() {
    let dir = TempDir::new();
    dir.dir("d/.git");
    dir.dir("d/.gitignore");
    let [path] = write_unformatted(&dir, ["d/a.nu"]);
    let output = format_in(&dir, &["d"]);
    assert_succeeds(&output);
    assert_eq!(read(&path), FORMATTED);
}

#[test]
fn format_takes_files_dot_ignore_negates_though_gitignore_ignores_them() {
    let dir = TempDir::new();
    dir.dir("d/.git");
    dir.file("d/.gitignore", "*.nu\n");
    dir.file("d/.ignore", "!a.nu\n");
    let [taken, ignored] = write_unformatted(&dir, ["d/a.nu", "d/b.nu"]);
    let output = format_in(&dir, &["d"]);
    assert_succeeds(&output);
    assert_eq!(read(&taken), FORMATTED);
    assert_eq!(read(&ignored), UNFORMATTED);
}

#[test]
fn format_takes_files_gitignore_negates_though_git_exclude_ignores_them() {
    let dir = TempDir::new();
    dir.file("d/.git/info/exclude", "*.nu\n");
    dir.file("d/.gitignore", "!a.nu\n");
    let [taken, ignored] = write_unformatted(&dir, ["d/a.nu", "d/b.nu"]);
    let output = format_in(&dir, &["d"]);
    assert_succeeds(&output);
    assert_eq!(read(&taken), FORMATTED);
    assert_eq!(read(&ignored), UNFORMATTED);
}

#[test]
fn format_takes_files_git_exclude_negates_though_global_excludes_file_ignores_them() {
    let dir = TempDir::new();
    dir.file("config/git/ignore", "*.nu\n");
    dir.file("d/.git/info/exclude", "!a.nu\n");
    let [taken, ignored] = write_unformatted(&dir, ["d/a.nu", "d/b.nu"]);
    let output = nuten_command()
        .args(["format", "d"])
        .env("XDG_CONFIG_HOME", dir.path("config"))
        .current_dir(&dir.0)
        .output()
        .unwrap();
    assert_succeeds(&output);
    assert_eq!(read(&taken), FORMATTED);
    assert_eq!(read(&ignored), UNFORMATTED);
}

#[test]
fn format_takes_files_inner_ignore_file_negates_though_outer_one_ignores_them() {
    let dir = TempDir::new();
    dir.dir("d/.git");
    dir.file("d/.gitignore", "*.nu\n");
    dir.file("d/e/.gitignore", "!a.nu\n");
    dir.file("d/.ignore", "x*.nu\n");
    dir.file("d/e/.ignore", "!xa.nu\n");
    let taken = write_unformatted(&dir, ["d/e/a.nu", "d/e/xa.nu"]);
    let ignored = write_unformatted(&dir, ["d/e/b.nu", "d/e/xb.nu"]);
    let output = format_in(&dir, &["d"]);
    assert_succeeds(&output);
    for path in &taken {
        assert_eq!(read(path), FORMATTED, "{}", path.display());
    }
    for path in &ignored {
        assert_eq!(read(path), UNFORMATTED, "{}", path.display());
    }
}

#[test]
fn format_reads_global_excludes_file_of_git_from_repository_root() {
    let dir = TempDir::new();
    dir.file("config/git/ignore", "/gen/\n");
    dir.dir("d/.git");
    let [ignored, taken] = write_unformatted(&dir, ["d/gen/a.nu", "d/e/gen/a.nu"]);
    let output = nuten_command()
        .args(["format", "d"])
        .env("XDG_CONFIG_HOME", dir.path("config"))
        .current_dir(&dir.0)
        .output()
        .unwrap();
    assert_succeeds(&output);
    assert_eq!(read(&ignored), UNFORMATTED);
    assert_eq!(read(&taken), FORMATTED);
}

#[test]
fn format_takes_no_global_excludes_file_of_git_outside_git_repository() {
    let dir = TempDir::new();
    dir.file("config/git/ignore", "a.nu\n");
    let [path] = write_unformatted(&dir, ["d/a.nu"]);
    let output = nuten_command()
        .args(["format", "d"])
        .env("XDG_CONFIG_HOME", dir.path("config"))
        .current_dir(&dir.0)
        .output()
        .unwrap();
    assert_succeeds(&output);
    assert_eq!(read(&path), FORMATTED);
}

#[test]
fn format_reads_global_excludes_file_from_git_config_global_over_system() {
    let dir = TempDir::new();
    dir.file("system", "[core]\n\texcludesFile = system-ignore\n");
    dir.file("system-ignore", "b.nu\n");
    dir.file("global", "[core]\n\texcludesFile = global-ignore\n");
    dir.file("global-ignore", "a.nu\n");
    dir.dir("d/.git");
    let [ignored, taken] = write_unformatted(&dir, ["d/a.nu", "d/b.nu"]);
    let output = nuten_command()
        .args(["format", "d"])
        .env("GIT_CONFIG_SYSTEM", dir.path("system"))
        .env("GIT_CONFIG_GLOBAL", dir.path("global"))
        .current_dir(&dir.0)
        .output()
        .unwrap();
    assert_succeeds(&output);
    assert_eq!(read(&ignored), UNFORMATTED);
    assert_eq!(read(&taken), FORMATTED);
}

#[test]
fn format_reads_global_excludes_file_at_path_relative_to_home_directory() {
    let dir = TempDir::new();
    dir.file("global", "[core]\n\texcludesFile = ~/ignore\n");
    dir.file("home/ignore", "a.nu\n");
    dir.dir("d/.git");
    let [ignored, taken] = write_unformatted(&dir, ["d/a.nu", "d/b.nu"]);
    let output = nuten_command()
        .args(["format", "d"])
        .env("GIT_CONFIG_GLOBAL", dir.path("global"))
        .env("HOME", dir.path("home"))
        .env("USERPROFILE", dir.path("home"))
        .current_dir(&dir.0)
        .output()
        .unwrap();
    assert_succeeds(&output);
    assert_eq!(read(&ignored), UNFORMATTED);
    assert_eq!(read(&taken), FORMATTED);
}

#[test]
fn format_reads_no_system_git_config_when_git_config_nosystem_is_set() {
    let dir = TempDir::new();
    dir.file("system", "[core]\n\texcludesFile = system-ignore\n");
    dir.file("system-ignore", "a.nu\n");
    dir.dir("d/.git");
    let [path] = write_unformatted(&dir, ["d/a.nu"]);
    let output = nuten_command()
        .args(["format", "d"])
        .env("GIT_CONFIG_SYSTEM", dir.path("system"))
        .current_dir(&dir.0)
        .output()
        .unwrap();
    assert_succeeds(&output);
    assert_eq!(read(&path), UNFORMATTED);
    let output = nuten_command()
        .args(["format", "d"])
        .env("GIT_CONFIG_SYSTEM", dir.path("system"))
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .current_dir(&dir.0)
        .output()
        .unwrap();
    assert_succeeds(&output);
    assert_eq!(read(&path), FORMATTED);
}

#[test]
fn format_formats_ignored_file_given_directly() {
    let dir = TempDir::new();
    dir.dir(".git");
    dir.file(".gitignore", "a.nu\n");
    let [path] = write_unformatted(&dir, ["a.nu"]);
    let output = format_in(&dir, &["a.nu"]);
    assert_succeeds(&output);
    assert_eq!(read(&path), FORMATTED);
}

#[test]
fn format_walks_directory_given_directly_though_ignored_or_version_control_directory() {
    let dir = TempDir::new();
    dir.dir(".git");
    dir.file(".gitignore", "gen/\n");
    let paths = write_unformatted(&dir, ["gen/a.nu", ".git/a.nu"]);
    let output = format_in(&dir, &["gen", ".git"]);
    assert_succeeds(&output);
    for path in &paths {
        assert_eq!(read(path), FORMATTED, "{}", path.display());
    }
}

/// Nothing when the test runs as root, which reads the file anyway.
#[cfg(unix)]
fn unreadable(dir: &TempDir, name: &str) -> Option<(PathBuf, String)> {
    use std::os::unix::fs::PermissionsExt;

    let path = dir.file(name, "a.nu\n");
    fs::set_permissions(&path, fs::Permissions::from_mode(0o000)).unwrap();
    let error = fs::read(&path).err()?;
    let line = io_error_line(&path, &error);
    Some((path, line))
}

/// Unreadable even to root, since a `.git` file not in UTF-8 names no git directory.
fn unreadable_dot_git(dir: &TempDir, name: &str) -> (PathBuf, String) {
    let path = dir.path(name);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(&path, b"\xff\n").unwrap();
    let error = fs::read_to_string(&path).unwrap_err();
    let line = io_error_line(&path, &error);
    (path, line)
}

/// A git config naming the global excludes file by a path with NUL, which no platform can read.
fn unreadable_global_excludes_file(dir: &TempDir) -> (PathBuf, PathBuf, String) {
    let config = dir.path("gitconfig");
    fs::write(&config, b"[core]\n\texcludesFile = a\0b\n").unwrap();
    let excludes = PathBuf::from("a\0b");
    let error = fs::read(&excludes).unwrap_err();
    let line = io_error_line(&excludes, &error);
    (config, excludes, line)
}

#[test]
fn format_reads_braces_in_ignore_file_as_characters() {
    let dir = TempDir::new();
    dir.file("d/.ignore", "a{b,c}.nu\n");
    let [ignored] = write_unformatted(&dir, ["d/a{b,c}.nu"]);
    let taken = write_unformatted(&dir, ["d/ab.nu", "d/ac.nu"]);
    let output = format_in(&dir, &["d"]);
    assert_succeeds(&output);
    assert_eq!(read(&ignored), UNFORMATTED);
    for path in &taken {
        assert_eq!(read(path), FORMATTED, "{}", path.display());
    }
}

#[test]
fn format_reads_lines_of_ignore_file_after_line_not_in_utf8() {
    let dir = TempDir::new();
    let ignore = dir.path("d/.ignore");
    fs::create_dir_all(dir.path("d")).unwrap();
    fs::write(&ignore, b"\xff\na.nu\n").unwrap();
    let [ignored, taken] = write_unformatted(&dir, ["d/a.nu", "d/b.nu"]);
    let output = format_in(&dir, &["d"]);
    assert_succeeds(&output);
    assert_eq!(read(&ignored), UNFORMATTED);
    assert_eq!(read(&taken), FORMATTED);
}

#[cfg(unix)]
#[test]
fn format_reports_ignore_file_it_cannot_read_and_leaves_files_under_it_alone() {
    let dir = TempDir::new();
    let skipped = write_unformatted(&dir, ["d/e/a.nu", "d/e/f/a.nu"]);
    let [taken] = write_unformatted(&dir, ["d/a.nu"]);
    let Some((_, line)) = unreadable(&dir, "d/e/.ignore") else {
        return;
    };
    let output = format_in(&dir, &["d"]);
    assert_eq!(output.status.code(), Some(2));
    assert_eq!(problems(&output), line);
    for path in &skipped {
        assert_eq!(read(path), UNFORMATTED, "{}", path.display());
    }
    assert_eq!(read(&taken), FORMATTED);
}

#[cfg(unix)]
#[test]
fn format_reports_each_file_for_ignores_it_cannot_read_in_one_directory() {
    let dir = TempDir::new();
    let (Some((_, dot_ignore)), Some((_, gitignore))) = (
        unreadable(&dir, "d/.ignore"),
        unreadable(&dir, "d/.gitignore"),
    ) else {
        return;
    };
    let (_, dot_git) = unreadable_dot_git(&dir, "d/.git");
    let output = format_in(&dir, &["d"]);
    assert_eq!(output.status.code(), Some(2));
    assert_eq!(problems(&output), dot_ignore + &gitignore + &dot_git);
}

#[test]
fn format_reports_dot_git_file_it_cannot_read_and_leaves_files_under_it_alone() {
    let dir = TempDir::new();
    let skipped = write_unformatted(&dir, ["d/e/a.nu", "d/e/f/a.nu"]);
    let [taken] = write_unformatted(&dir, ["d/a.nu"]);
    let (_, line) = unreadable_dot_git(&dir, "d/e/.git");
    let output = format_in(&dir, &["d"]);
    assert_eq!(output.status.code(), Some(2));
    assert_eq!(problems(&output), line);
    for path in &skipped {
        assert_eq!(read(path), UNFORMATTED, "{}", path.display());
    }
    assert_eq!(read(&taken), FORMATTED);
}

#[test]
fn format_leaves_directory_alone_when_it_cannot_read_dot_git_file_above_it() {
    let dir = TempDir::new();
    let [skipped] = write_unformatted(&dir, ["d/e/a.nu"]);
    let (_, line) = unreadable_dot_git(&dir, "d/.git");
    let output = format_in(&dir, &["d/e"]);
    assert_eq!(output.status.code(), Some(2));
    assert_eq!(problems(&output), line);
    assert_eq!(read(&skipped), UNFORMATTED);
}

#[cfg(unix)]
#[test]
fn format_leaves_directory_given_alone_when_it_cannot_read_its_ignore_file() {
    let dir = TempDir::new();
    let [skipped, taken] = write_unformatted(&dir, ["d/a.nu", "b.nu"]);
    let Some((_, line)) = unreadable(&dir, "d/.ignore") else {
        return;
    };
    let output = format_in(&dir, &["d", "b.nu"]);
    assert_eq!(output.status.code(), Some(2));
    assert_eq!(problems(&output), line);
    assert_eq!(read(&skipped), UNFORMATTED);
    assert_eq!(read(&taken), FORMATTED);
}

#[cfg(unix)]
#[test]
fn format_leaves_directory_alone_when_it_cannot_read_ignore_file_above_it() {
    let dir = TempDir::new();
    let [skipped] = write_unformatted(&dir, ["d/e/a.nu"]);
    let Some((_, line)) = unreadable(&dir, "d/.ignore") else {
        return;
    };
    let output = format_in(&dir, &["d/e"]);
    assert_eq!(output.status.code(), Some(2));
    assert_eq!(problems(&output), line);
    assert_eq!(read(&skipped), UNFORMATTED);
}

#[test]
fn format_leaves_directories_alone_when_it_cannot_read_global_excludes_file_of_git() {
    let dir = TempDir::new();
    let [skipped, taken] = write_unformatted(&dir, ["d/a.nu", "b.nu"]);
    let (config, _, line) = unreadable_global_excludes_file(&dir);
    let output = nuten_command()
        .args(["format", "d", "b.nu"])
        .env("GIT_CONFIG_GLOBAL", config)
        .current_dir(&dir.0)
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    assert_eq!(problems(&output), line);
    assert_eq!(read(&skipped), UNFORMATTED);
    assert_eq!(read(&taken), FORMATTED);
}

#[test]
fn format_reports_problem_of_directory_once_when_given_twice() {
    let dir = TempDir::new();
    let (_, line) = unreadable_dot_git(&dir, "d/.git");
    let output = format_in(&dir, &["d", "d"]);
    assert_eq!(output.status.code(), Some(2));
    assert_eq!(problems(&output), line);
}

#[cfg(unix)]
#[test]
fn format_reports_problem_of_ignore_file_once_when_above_one_directory_and_in_another() {
    let dir = TempDir::new();
    if unreadable(&dir, "d/.ignore").is_none() {
        return;
    }
    dir.dir("d/e");
    for paths in [["d", "d/e"], ["d/e", "d"]] {
        let output = format_in(&dir, &paths);
        assert_eq!(output.status.code(), Some(2));
        let problems = problems(&output);
        assert_eq!(problems.lines().count(), 1, "{paths:?}: {problems}");
    }
}

#[cfg(unix)]
#[test]
fn format_reports_unreadable_current_directory_by_dot() {
    use std::os::unix::fs::PermissionsExt;

    let dir = TempDir::new();
    fs::set_permissions(&dir.0, fs::Permissions::from_mode(0o300)).unwrap();
    let error = fs::read_dir(&dir.0).err();
    let outputs = [&["."][..], &["./"], &[".", "./"]].map(|paths| format_in(&dir, paths));
    fs::set_permissions(&dir.0, fs::Permissions::from_mode(0o755)).unwrap();
    // Root reads a directory without permission, so the test proves nothing then.
    let Some(error) = error else {
        return;
    };
    for output in &outputs {
        assert_eq!(output.status.code(), Some(2));
        assert_eq!(problems(output), io_error_line(Path::new("."), &error));
    }
}

#[cfg(unix)]
#[test]
fn format_reports_unreadable_directory_once_when_given_with_dot_or_trailing_slash() {
    use std::os::unix::fs::PermissionsExt;

    let dir = TempDir::new();
    let unreadable = dir.dir("u");
    fs::set_permissions(&unreadable, fs::Permissions::from_mode(0o300)).unwrap();
    let error = fs::read_dir(&unreadable).err();
    let output = format_in(&dir, &["u", "u/", "u/.", "./u/"]);
    fs::set_permissions(&unreadable, fs::Permissions::from_mode(0o755)).unwrap();
    // Root reads a directory without permission, so the test proves nothing then.
    let Some(error) = error else {
        return;
    };
    assert_eq!(output.status.code(), Some(2));
    assert_eq!(problems(&output), io_error_line(Path::new("u"), &error));
}

#[cfg(unix)]
#[test]
fn format_walks_no_symlink_under_directory() {
    let dir = TempDir::new();
    let outside = write_unformatted(&dir, ["out/a.nu", "out/e/b.nu"]);
    dir.dir("d");
    std::os::unix::fs::symlink(&outside[0], dir.path("d/link.nu")).unwrap();
    std::os::unix::fs::symlink(dir.path("out/e"), dir.path("d/link")).unwrap();
    let output = format_in(&dir, &["d"]);
    assert_succeeds(&output);
    for path in &outside {
        assert_eq!(read(path), UNFORMATTED, "{}", path.display());
    }
}

#[cfg(unix)]
#[test]
fn format_walks_symlink_to_directory_given_directly() {
    let dir = TempDir::new();
    let [path] = write_unformatted(&dir, ["real/a.nu"]);
    std::os::unix::fs::symlink(dir.path("real"), dir.path("link")).unwrap();
    let output = format_in(&dir, &["link"]);
    assert_succeeds(&output);
    assert_eq!(read(&path), FORMATTED);
}

#[cfg(unix)]
#[test]
fn format_skips_fifo_under_directory_without_waiting() {
    let dir = TempDir::new();
    fifo(&dir.dir("d").join("fifo.nu"));
    let [path] = write_unformatted(&dir, ["d/a.nu"]);
    let output = format_in_time(&[&dir.path("d")]);
    assert_succeeds(&output);
    assert_eq!(read(&path), FORMATTED);
}

#[test]
fn format_succeeds_on_directory_without_nu_files() {
    let dir = TempDir::new();
    dir.file("d/a.txt", UNFORMATTED);
    dir.dir("e");
    let output = format_in(&dir, &["d", "e"]);
    assert_succeeds(&output);
}

#[cfg(unix)]
#[test]
fn format_reports_files_under_directory_in_path_order_where_directory_is_given() {
    let dir = TempDir::new();
    for name in ["z.nu", "d/b.nu", "d/a.nu", "d/a/c.nu", "y.nu"] {
        dir.file(name, "ls ||");
    }
    let output = format_in(&dir, &["z.nu", "d", "y.nu"]);
    assert_eq!(output.status.code(), Some(2));
    let problems = problems(&output);
    let heads: Vec<_> = problems
        .lines()
        .map(|line| &line[..line.find(':').unwrap()])
        .collect();
    assert_eq!(
        heads,
        ["z.nu", "d/a/c.nu", "d/a.nu", "d/b.nu", "y.nu"],
        "{problems}"
    );
}

#[cfg(unix)]
#[test]
fn format_reports_files_under_current_directory_by_paths_relative_to_it() {
    let dir = TempDir::new();
    dir.file("a/bad.nu", "ls ||");
    for given in [".", "./"] {
        let output = format_in(&dir, &[given]);
        assert_eq!(output.status.code(), Some(2));
        let problems = problems(&output);
        assert!(
            problems.starts_with("a/bad.nu:1:4: "),
            "{given}: {problems}"
        );
    }
}

#[cfg(unix)]
#[test]
fn format_reports_file_under_overlapping_directories_once_at_first_path() {
    let dir = TempDir::new();
    dir.file("d/e/bad.nu", "ls ||");
    let output = format_in(&dir, &["d/e", "d", "d/e/bad.nu"]);
    assert_eq!(output.status.code(), Some(2));
    let problems = problems(&output);
    assert_eq!(problems.lines().count(), 1, "{problems}");
    assert!(problems.starts_with("d/e/bad.nu:1:4: "), "{problems}");
}

#[test]
fn format_takes_nearest_config_file_for_each_file_under_directory() {
    let dir = TempDir::new();
    dir.file("d/nuten.nuon", "{indent_width: 2}");
    dir.file("d/e/nuten.nuon", "{indent_width: 3}");
    let top = dir.file("d/a.nu", BLOCK);
    let inner = dir.file("d/e/a.nu", BLOCK);
    let output = format_in(&dir, &["d"]);
    assert_succeeds(&output);
    assert_eq!(read(&top), "if true {\n  ls\n}\n");
    assert_eq!(read(&inner), "if true {\n   ls\n}\n");
}

#[test]
fn format_formats_config_file_under_directory_after_reading_it() {
    let dir = TempDir::new();
    let config = dir.file("d/nuten.nuon", "{indent_width: 2}");
    let path = dir.file("d/a.nu", BLOCK);
    let output = format_in(&dir, &["d"]);
    assert_succeeds(&output);
    assert_eq!(read(&config), "{ indent_width: 2 }\n");
    assert_eq!(read(&path), "if true {\n  ls\n}\n");
}

#[test]
fn format_writes_nothing_when_config_file_under_directory_is_wrong() {
    let dir = TempDir::new();
    let nuon = dir.file("d/e/nuten.nuon", "{a: 1}");
    let paths = write_unformatted(&dir, ["d/a.nu", "d/e/a.nu"]);
    let output = format_in(&dir, &["d"]);
    assert_eq!(output.status.code(), Some(2));
    assert_eq!(
        problems(&output),
        format!("{}:1:2: unknown key `a`\n", nuon.display())
    );
    for path in &paths {
        assert_eq!(read(path), UNFORMATTED, "{}", path.display());
    }
}

#[cfg(unix)]
#[test]
fn format_reports_unreadable_directory_under_directory_and_continues() {
    use std::os::unix::fs::PermissionsExt;

    let dir = TempDir::new();
    let [locked, taken] = write_unformatted(&dir, ["d/locked/a.nu", "d/b.nu"]);
    let locked_dir = dir.path("d/locked");
    fs::set_permissions(&locked_dir, fs::Permissions::from_mode(0o000)).unwrap();
    let error = fs::read_dir(&locked_dir).err();
    let output = format_in(&dir, &["d"]);
    fs::set_permissions(&locked_dir, fs::Permissions::from_mode(0o755)).unwrap();
    // Root reads a directory without permission, so the test proves nothing then.
    let Some(error) = error else {
        return;
    };
    assert_eq!(output.status.code(), Some(2));
    assert_eq!(
        problems(&output),
        io_error_line(Path::new("d/locked"), &error)
    );
    assert_eq!(read(&locked), UNFORMATTED);
    assert_eq!(read(&taken), FORMATTED);
}

#[cfg(unix)]
#[test]
fn format_reports_problems_of_walking_and_of_formatting_in_order_of_paths_given() {
    let dir = TempDir::new();
    dir.file("z.nu", "ls ||");
    dir.file("y.nu", "ls ||");
    let (dot_git, _) = unreadable_dot_git(&dir, "d/.git");
    let dot_git = dot_git.display().to_string();
    for (paths, expected) in [
        (["z.nu", "d", "y.nu"], ["z.nu", &dot_git, "y.nu"]),
        (["y.nu", "z.nu", "d"], ["y.nu", "z.nu", &dot_git]),
    ] {
        let output = format_in(&dir, &paths);
        assert_eq!(output.status.code(), Some(2));
        assert_eq!(reported_paths(&output), expected, "{}", stderr(&output));
    }
}

#[cfg(unix)]
#[test]
fn format_reports_problems_met_while_walking_directory_among_its_files() {
    let dir = TempDir::new();
    dir.file("d/a.nu", "ls ||");
    let (dot_git, _) = unreadable_dot_git(&dir, "d/b/.git");
    let dot_git = dot_git.display().to_string();
    dir.file("d/b/x.nu", "ls ||");
    dir.file("d/c.nu", "ls ||");
    let output = format_in(&dir, &["d"]);
    assert_eq!(output.status.code(), Some(2));
    assert_eq!(
        reported_paths(&output),
        ["d/a.nu", &dot_git, "d/c.nu"],
        "{}",
        problems(&output)
    );
}

#[cfg(unix)]
#[test]
fn format_reports_path_it_cannot_resolve_where_it_is_given() {
    let dir = TempDir::new();
    dir.file("z.nu", "ls ||");
    dir.file("y.nu", "ls ||");
    let output = format_in(&dir, &["z.nu", "missing/a.nu", "y.nu"]);
    assert_eq!(output.status.code(), Some(2));
    assert_eq!(
        reported_paths(&output),
        ["z.nu", "missing/a.nu", "y.nu"],
        "{}",
        problems(&output)
    );
}

#[cfg(unix)]
#[test]
fn format_reports_wrong_config_file_once_where_first_path_it_configures_is_given() {
    let dir = TempDir::new();
    let (dot_git, _) = unreadable_dot_git(&dir, "d/.git");
    let dot_git = dot_git.display().to_string();
    let nuon = dir.file("e/nuten.nuon", "{a: 1}").display().to_string();
    let paths = write_unformatted(&dir, ["e/a.nu", "e/b.nu"]);
    for (args, expected) in [
        (
            &["d", "e/a.nu", "e/b.nu"][..],
            &[dot_git.as_str(), &nuon][..],
        ),
        (&["e/b.nu", "d", "e/a.nu"], &[&nuon, &dot_git]),
        (&["e", "d"], &[&nuon, &dot_git]),
        (&["d", "e"], &[&dot_git, &nuon]),
        (&["e/a.nu", "e"], &[&nuon]),
    ] {
        let output = format_in(&dir, args);
        assert_eq!(output.status.code(), Some(2));
        assert_eq!(reported_paths(&output), expected, "{}", stderr(&output));
        for path in &paths {
            assert_eq!(read(path), UNFORMATTED, "{}", path.display());
        }
    }
}

#[cfg(unix)]
#[test]
fn format_reports_problem_of_global_excludes_file_once_at_first_directory_respecting_it() {
    let dir = TempDir::new();
    let (config, excludes, _) = unreadable_global_excludes_file(&dir);
    let excludes = excludes.display().to_string();
    dir.file("z.nu", "ls ||");
    dir.file("off/nuten.nuon", "{respect_gitignore: false}");
    dir.file("off/y.nu", "ls ||");
    let skipped = write_unformatted(&dir, ["on1/a.nu", "on2/a.nu"]);
    let output = nuten_command()
        .args(["format", "z.nu", "off", "on1", "on2"])
        .env("GIT_CONFIG_GLOBAL", config)
        .current_dir(&dir.0)
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    assert_eq!(
        reported_paths(&output),
        ["z.nu", "off/y.nu", &excludes],
        "{}",
        problems(&output)
    );
    for path in &skipped {
        assert_eq!(read(path), UNFORMATTED, "{}", path.display());
    }
}

#[test]
fn format_formats_ignored_files_when_respect_gitignore_is_off() {
    let dir = TempDir::new();
    dir.file("d/nuten.nuon", "{respect_gitignore: false}");
    dir.dir("d/.git");
    dir.file("d/.gitignore", "gen/\n");
    dir.file("d/.ignore", "local.nu\n");
    let taken = write_unformatted(&dir, ["d/gen/a.nu", "d/local.nu"]);
    let [vcs] = write_unformatted(&dir, ["d/.git/a.nu"]);
    let output = format_in(&dir, &["d"]);
    assert_succeeds(&output);
    for path in &taken {
        assert_eq!(read(path), FORMATTED, "{}", path.display());
    }
    assert_eq!(read(&vcs), UNFORMATTED);
}

#[cfg(unix)]
#[test]
fn format_reads_no_ignore_file_when_respect_gitignore_is_off() {
    let dir = TempDir::new();
    dir.file("d/nuten.nuon", "{respect_gitignore: false}");
    let [path] = write_unformatted(&dir, ["d/a.nu"]);
    if unreadable(&dir, "d/.ignore").is_none() || unreadable(&dir, "config/git/ignore").is_none() {
        return;
    }
    let output = nuten_command()
        .args(["format", "d"])
        .env("XDG_CONFIG_HOME", dir.path("config"))
        .current_dir(&dir.0)
        .output()
        .unwrap();
    assert_succeeds(&output);
    assert_eq!(read(&path), FORMATTED);
}

#[test]
fn format_takes_respect_gitignore_from_config_file_nearest_directory_given() {
    let dir = TempDir::new();
    dir.file("d/nuten.nuon", "{respect_gitignore: false}");
    dir.file("d/e/nuten.nuon", "{respect_gitignore: true}");
    dir.dir("d/.git");
    dir.file("d/.gitignore", "gen/\n");
    let [path] = write_unformatted(&dir, ["d/e/gen/a.nu"]);
    let output = format_in(&dir, &["d/e"]);
    assert_succeeds(&output);
    assert_eq!(read(&path), UNFORMATTED);
    let output = format_in(&dir, &["d"]);
    assert_succeeds(&output);
    assert_eq!(read(&path), FORMATTED);
}

#[cfg(unix)]
#[test]
fn format_takes_respect_gitignore_from_config_file_where_symlinked_directory_leads() {
    let dir = TempDir::new();
    dir.file("real/nuten.nuon", "{respect_gitignore: false}");
    dir.file("real/sub/.ignore", "a.nu\n");
    let [path] = write_unformatted(&dir, ["real/sub/a.nu"]);
    dir.file("e/nuten.nuon", "{respect_gitignore: true}");
    std::os::unix::fs::symlink(dir.path("real/sub"), dir.path("e/link")).unwrap();
    let output = format_in(&dir, &["e/link"]);
    assert_succeeds(&output);
    assert_eq!(read(&path), FORMATTED);
}

#[test]
fn format_reads_no_global_excludes_file_of_git_when_respect_gitignore_is_off() {
    let dir = TempDir::new();
    dir.file("off/nuten.nuon", "{respect_gitignore: false}");
    let [on, off] = write_unformatted(&dir, ["on/a.nu", "off/a.nu"]);
    let (config, _, _) = unreadable_global_excludes_file(&dir);
    let output = nuten_command()
        .args(["format", "on", "off"])
        .env("GIT_CONFIG_GLOBAL", config)
        .current_dir(&dir.0)
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    let problems = problems(&output);
    assert_eq!(problems.lines().count(), 1, "{problems}");
    assert_eq!(read(&on), UNFORMATTED);
    assert_eq!(read(&off), FORMATTED);
}

#[test]
fn format_takes_respect_gitignore_from_given_config_file() {
    let dir = TempDir::new();
    dir.file("d/nuten.nuon", "{respect_gitignore: true}");
    dir.file("off.nuon", "{respect_gitignore: false}");
    dir.file("d/.ignore", "a.nu\n");
    let [path] = write_unformatted(&dir, ["d/a.nu"]);
    let output = format_in(&dir, &["-c", "off.nuon", "d"]);
    assert_succeeds(&output);
    assert_eq!(read(&path), FORMATTED);
}

#[test]
fn format_reads_no_config_file_of_directory_when_given_config_file() {
    let dir = TempDir::new();
    dir.file("d/nuten.nuon", "{ indent_width: 8 }\n");
    dir.file("two.nuon", "{indent_width: 2}");
    let path = dir.file("d/a.nu", BLOCK);
    let output = format_in(&dir, &["-c", "two.nuon", "d"]);
    assert_succeeds(&output);
    assert_eq!(read(&path), "if true {\n  ls\n}\n");
}

#[test]
fn format_reports_missing_file_once_when_given_config_file() {
    let dir = TempDir::new();
    dir.file("two.nuon", "{indent_width: 2}");
    let error = fs::read(dir.path("missing.nu")).unwrap_err();
    let output = format_in(&dir, &["-c", "two.nuon", "missing.nu"]);
    assert_eq!(output.status.code(), Some(2));
    assert_eq!(
        problems(&output),
        io_error_line(Path::new("missing.nu"), &error)
    );
}

#[test]
fn format_writes_nothing_when_config_file_of_directory_is_wrong() {
    let dir = TempDir::new();
    let nuon = dir.file("d/nuten.nuon", "{respect_gitignore: 1}");
    let paths = write_unformatted(&dir, ["d/a.nu", "a.nu"]);
    let output = format_in(&dir, &["a.nu", "d"]);
    assert_eq!(output.status.code(), Some(2));
    assert_eq!(
        problems(&output),
        format!(
            "{}:1:21: `respect_gitignore` takes a boolean, not an integer\n",
            nuon.display()
        )
    );
    for path in &paths {
        assert_eq!(read(path), UNFORMATTED, "{}", path.display());
    }
}
