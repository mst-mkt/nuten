#[cfg(unix)]
use std::io;
use std::{fs, path::Path};

#[cfg(unix)]
use crate::support::{BLOCK, fifo, format_in_time};
use crate::support::{
    FORMATTED, TempDir, UNFORMATTED, assert_succeeds, format, format_in, io_error_line,
    nuten_command, problems, read, stderr, stdout, write_unformatted,
};

/// Sorted, since `read_dir` yields them in no set order.
fn names_in(dir: &Path) -> Vec<String> {
    let mut names: Vec<_> = fs::read_dir(dir)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().into_string().unwrap())
        .collect();
    names.sort_unstable();
    names
}

#[test]
fn format_rewrites_each_file_in_place() {
    let dir = TempDir::new();
    let a = dir.file("a.nu", "ls|first");
    let b = dir.file("b.nu", "echo   a\n\n\n");
    let output = format(&[&a, &b]);
    assert_succeeds(&output);
    assert_eq!(stdout(&output), "");
    assert_eq!(read(&a), "ls | first\n");
    assert_eq!(read(&b), "echo a\n");
}

#[test]
fn format_leaves_formatted_file_alone() {
    let dir = TempDir::new();
    let path = dir.file("a.nu", "ls | first\n");
    // Read-only, so that writing it would fail.
    let mut permissions = fs::metadata(&path).unwrap().permissions();
    permissions.set_readonly(true);
    fs::set_permissions(&path, permissions).unwrap();
    let output = format(&[&path]);
    assert_succeeds(&output);
}

#[test]
fn format_leaves_nothing_beside_file_it_rewrites() {
    let dir = TempDir::new();
    let [path] = write_unformatted(&dir, ["a.nu"]);
    let output = format(&[&path]);
    assert_succeeds(&output);
    assert_eq!(read(&path), FORMATTED);
    assert_eq!(names_in(&dir.0), ["a.nu"]);
}

#[test]
fn format_reports_read_only_file_and_leaves_its_directory_unchanged() {
    let dir = TempDir::new();
    let [path] = write_unformatted(&dir, ["a.nu"]);
    let mut permissions = fs::metadata(&path).unwrap().permissions();
    permissions.set_readonly(true);
    fs::set_permissions(&path, permissions).unwrap();
    // Root writes a read-only file, so the test proves nothing then.
    let Some(error) = fs::OpenOptions::new().write(true).open(&path).err() else {
        return;
    };
    let output = format(&[&path]);
    assert_eq!(output.status.code(), Some(2));
    assert_eq!(problems(&output), io_error_line(&path, &error));
    assert_eq!(read(&path), UNFORMATTED);
    assert_eq!(names_in(&dir.0), ["a.nu"]);
}

#[test]
fn format_rewrites_file_under_each_of_its_hard_links() {
    let dir = TempDir::new();
    let [path] = write_unformatted(&dir, ["a.nu"]);
    let other = dir.path("b.nu");
    fs::hard_link(&path, &other).unwrap();
    let output = format(&[&path]);
    assert_succeeds(&output);
    assert_eq!(read(&other), FORMATTED);
    assert_eq!(names_in(&dir.0), ["a.nu", "b.nu"]);
}

#[cfg(unix)]
#[test]
fn format_keeps_permissions_of_file_it_rewrites() {
    use std::os::unix::fs::PermissionsExt;

    let dir = TempDir::new();
    let [path] = write_unformatted(&dir, ["a.nu"]);
    fs::set_permissions(&path, fs::Permissions::from_mode(0o751)).unwrap();
    let output = format(&[&path]);
    assert_succeeds(&output);
    assert_eq!(read(&path), FORMATTED);
    let mode = fs::metadata(&path).unwrap().permissions().mode();
    assert_eq!(mode & 0o7777, 0o751);
}

#[cfg(unix)]
#[test]
fn format_rewrites_file_through_symlink_and_keeps_symlink() {
    let dir = TempDir::new();
    let [path] = write_unformatted(&dir, ["a.nu"]);
    let link = dir.path("link.nu");
    std::os::unix::fs::symlink(&path, &link).unwrap();
    let output = format(&[&link]);
    assert_succeeds(&output);
    assert!(fs::symlink_metadata(&link).unwrap().is_symlink());
    assert_eq!(read(&path), FORMATTED);
    assert_eq!(names_in(&dir.0), ["a.nu", "link.nu"]);
}

#[cfg(unix)]
#[test]
fn format_rewrites_file_in_directory_it_cannot_write() {
    use std::os::unix::fs::PermissionsExt;

    let dir = TempDir::new();
    let [path] = write_unformatted(&dir, ["d/a.nu"]);
    let locked = dir.path("d");
    fs::set_permissions(&locked, fs::Permissions::from_mode(0o555)).unwrap();
    let denied = fs::File::create(locked.join("probe")).is_err();
    let output = format(&[&path]);
    // Made writable again before any check, so that the directory is removed even when one fails.
    fs::set_permissions(&locked, fs::Permissions::from_mode(0o755)).unwrap();
    // Root writes a directory without permission, so the test proves nothing then.
    if !denied {
        return;
    }
    assert_succeeds(&output);
    assert_eq!(read(&path), FORMATTED);
    assert_eq!(names_in(&locked), ["a.nu"]);
}

#[cfg(unix)]
#[test]
fn format_rewrites_file_whose_name_leaves_no_room_for_backup() {
    let dir = TempDir::new();
    // 250 bytes, which fits in NAME_MAX but leaves no room for a backup's suffix.
    let name = "a".repeat(247) + ".nu";
    let path = dir.path(&name);
    // A filesystem with shorter names holds no such file, so the test proves nothing then.
    match fs::write(&path, UNFORMATTED) {
        Err(error) if error.kind() == io::ErrorKind::InvalidFilename => return,
        written => written.unwrap(),
    }
    let output = format(&[&path]);
    assert_succeeds(&output);
    assert_eq!(read(&path), FORMATTED);
    assert_eq!(names_in(&dir.0), [name]);
}

#[cfg(target_os = "linux")]
#[test]
fn format_exits_with_2_though_it_cannot_write_problems() {
    let dir = TempDir::new();
    dir.file("bad.nu", "ls || x\n");
    let full = fs::File::options().write(true).open("/dev/full").unwrap();
    let output = nuten_command()
        .args(["format", "bad.nu"])
        .current_dir(&dir.0)
        .stderr(full)
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
}

#[test]
fn format_marks_each_problem_as_error() {
    let dir = TempDir::new();
    let bad = dir.file("bad.nu", "ls || x\nls || y\n");
    let missing = dir.path("missing.nu");
    let output = format(&[&bad, &missing]);
    assert_eq!(output.status.code(), Some(2));
    let stderr = stderr(&output);
    let lines: Vec<_> = stderr.lines().collect();
    // The last is the summary.
    let (_, problems) = lines.split_last().unwrap();
    assert_eq!(problems.len(), 3, "{stderr}");
    assert!(
        problems.iter().all(|line| line.starts_with("error: ")),
        "{stderr}"
    );
}

#[test]
fn format_colors_error_mark_when_clicolor_force_is_set() {
    let dir = TempDir::new();
    dir.file("bad.nu", "ls || x\n");
    let output = nuten_command()
        .args(["format", "bad.nu"])
        .current_dir(&dir.0)
        .env("CLICOLOR_FORCE", "1")
        .output()
        .unwrap();
    let stderr = stderr(&output);
    assert!(
        stderr.starts_with("\u{1b}[1m\u{1b}[31merror:\u{1b}[0m bad.nu:1:4: "),
        "{stderr}"
    );
}

#[test]
fn format_leaves_error_mark_plain_when_no_color_is_set_though_clicolor_force_is_set() {
    let dir = TempDir::new();
    dir.file("bad.nu", "ls || x\n");
    let output = nuten_command()
        .args(["format", "bad.nu"])
        .current_dir(&dir.0)
        .env("CLICOLOR_FORCE", "1")
        .env("NO_COLOR", "1")
        .output()
        .unwrap();
    let stderr = stderr(&output);
    assert!(stderr.starts_with("error: bad.nu:1:4: "), "{stderr}");
}

#[test]
fn format_reports_syntax_error_position_and_continues() {
    let dir = TempDir::new();
    let bad = dir.file("bad.nu", "ls\n\n  ls || x\n");
    let good = dir.file("good.nu", "ls|first");
    let output = format(&[&bad, &good]);
    assert_eq!(output.status.code(), Some(2));
    let problems = problems(&output);
    assert_eq!(problems.lines().count(), 1, "{problems}");
    assert!(
        problems.starts_with(&format!("{}:3:6: ", bad.display())),
        "{problems}"
    );
    assert_eq!(read(&bad), "ls\n\n  ls || x\n");
    assert_eq!(read(&good), "ls | first\n");
}

#[test]
fn format_reports_unreadable_file() {
    let dir = TempDir::new();
    let missing = dir.path("missing.nu");
    let output = format(&[&missing]);
    assert_eq!(output.status.code(), Some(2));
    assert!(problems(&output).starts_with(&format!("{}: ", missing.display())));
}

#[cfg(unix)]
#[test]
fn format_reports_fifo_without_waiting_and_continues() {
    let dir = TempDir::new();
    let fifo_path = dir.path("fifo.nu");
    fifo(&fifo_path);
    let good = dir.file("good.nu", "ls|first");
    let output = format_in_time(&[&fifo_path, &good]);
    assert_eq!(output.status.code(), Some(2));
    assert_eq!(
        problems(&output),
        format!("{}: not a regular file\n", fifo_path.display())
    );
    assert_eq!(read(&good), "ls | first\n");
}

#[test]
fn format_rejects_unknown_flag() {
    let dir = TempDir::new();
    let [path] = write_unformatted(&dir, ["a.nu"]);
    let output = format_in(&dir, &["--bogus"]);
    assert_eq!(output.status.code(), Some(2));
    let stderr = stderr(&output);
    assert!(stderr.contains("--bogus"), "{stderr}");
    assert!(!stderr.contains("No such file"), "{stderr}");
    assert_eq!(read(&path), UNFORMATTED);
}

#[test]
fn format_takes_file_named_like_subcommand() {
    let dir = TempDir::new();
    let path = dir.file("format", "ls|first");
    let output = format(&[&path]);
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    assert_eq!(read(&path), "ls | first\n");
}

#[test]
fn format_reports_files_in_order_given() {
    let dir = TempDir::new();
    // Many, so that they are spread over threads, and named against their order.
    let bad: Vec<_> = (0..64)
        .rev()
        .map(|index| dir.file(&format!("bad{index:02}.nu"), "ls ||"))
        .collect();
    let good: Vec<_> = (0..64)
        .map(|index| dir.file(&format!("good{index:02}.nu"), "ls|first"))
        .collect();
    let paths: Vec<_> = bad
        .iter()
        .zip(&good)
        .flat_map(|(bad, good)| [bad.as_path(), good.as_path()])
        .collect();
    let output = format(&paths);
    assert_eq!(output.status.code(), Some(2));
    let expected: Vec<_> = bad
        .iter()
        .map(|path| format!("{}:1:4", path.display()))
        .collect();
    let problems = problems(&output);
    let heads: Vec<_> = problems
        .lines()
        .map(|line| &line[..line.find(": ").unwrap()])
        .collect();
    assert_eq!(heads, expected, "{problems}");
    for path in &good {
        assert_eq!(read(path), "ls | first\n");
    }
}

#[test]
fn format_reports_every_syntax_error_of_file_in_order_of_position() {
    let dir = TempDir::new();
    let bad = dir.file("bad.nu", "ls ||\n\n日 || x\n");
    let output = format(&[&bad]);
    assert_eq!(output.status.code(), Some(2));
    let problems = problems(&output);
    let heads: Vec<_> = problems
        .lines()
        .map(|line| &line[..line.find(": ").unwrap()])
        .collect();
    assert_eq!(
        heads,
        [
            format!("{}:1:4", bad.display()),
            format!("{}:3:3", bad.display())
        ],
        "{problems}"
    );
}

#[test]
fn format_reports_file_given_by_several_paths_once() {
    let dir = TempDir::new();
    dir.dir("a");
    let bad = dir.file("bad.nu", "ls ||");
    let output = format_in(&dir, &["bad.nu", "a/../bad.nu", "bad.nu"]);
    assert_eq!(output.status.code(), Some(2));
    let problems = problems(&output);
    assert_eq!(problems.lines().count(), 1, "{problems}");
    assert!(problems.starts_with("bad.nu:1:4: "), "{problems}");
    assert_eq!(read(&bad), "ls ||");
}

#[test]
fn format_reports_missing_file_once_when_given_twice() {
    let dir = TempDir::new();
    let error = fs::read(dir.path("missing.nu")).unwrap_err();
    let output = format_in(&dir, &["missing.nu", "missing.nu"]);
    assert_eq!(output.status.code(), Some(2));
    assert_eq!(
        problems(&output),
        io_error_line(Path::new("missing.nu"), &error)
    );
}

#[test]
fn format_reports_each_missing_file() {
    let dir = TempDir::new();
    let [a, b] = ["a.nu", "b.nu"].map(|name| fs::read(dir.path(name)).unwrap_err());
    let output = format_in(&dir, &["a.nu", "b.nu"]);
    assert_eq!(output.status.code(), Some(2));
    assert_eq!(
        problems(&output),
        io_error_line(Path::new("a.nu"), &a) + &io_error_line(Path::new("b.nu"), &b)
    );
}

#[cfg(unix)]
#[test]
fn format_reports_file_given_with_symlink_to_it_once_at_first_path() {
    let dir = TempDir::new();
    let bad = dir.file("bad.nu", "ls ||");
    std::os::unix::fs::symlink(&bad, dir.path("link.nu")).unwrap();
    let output = format_in(&dir, &["link.nu", "bad.nu"]);
    assert_eq!(output.status.code(), Some(2));
    let problems = problems(&output);
    assert_eq!(problems.lines().count(), 1, "{problems}");
    assert!(problems.starts_with("link.nu:1:4: "), "{problems}");
}

#[cfg(unix)]
#[test]
fn format_formats_hard_linked_file_once_with_config_of_first_path() {
    let dir = TempDir::new();
    dir.file("a/nuten.nuon", "{indent_width: 2}");
    dir.file("b/nuten.nuon", "{indent_width: 3}");
    let path = dir.file("a/a.nu", BLOCK);
    fs::hard_link(&path, dir.path("b/a.nu")).unwrap();
    let output = format_in(&dir, &["a/a.nu", "b/a.nu"]);
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    assert_eq!(read(&path), "if true {\n  ls\n}\n");
}
