#[cfg(unix)]
use std::{fs, process::Command};
use std::{io, path::Path, process::Stdio, thread};

use crate::support::{
    BLOCK, FORMATTED, TempDir, UNFORMATTED, assert_succeeds_quietly, format_in,
    format_stdin_command, format_stdin_in, nuten_command, problems, read, stderr, stdout,
    view_of_unformatted, write_unformatted,
};

#[test]
fn format_writes_stdin_formatted_to_stdout() {
    let dir = TempDir::new();
    let output = format_stdin_in(&dir, &["-"], UNFORMATTED.as_bytes());
    assert_succeeds_quietly(&output);
    assert_eq!(stdout(&output), FORMATTED);
}

#[test]
fn format_takes_config_file_nearest_current_directory_for_stdin() {
    let dir = TempDir::new();
    dir.file("nuten.nuon", "{indent_width: 2}");
    let output = format_stdin_in(&dir, &["-"], BLOCK.as_bytes());
    assert_succeeds_quietly(&output);
    assert_eq!(stdout(&output), "if true {\n  ls\n}\n");
}

#[cfg(unix)]
#[test]
fn format_takes_config_file_nearest_absolute_stdin_filepath_in_removed_current_directory() {
    let dir = TempDir::new();
    dir.file("nuten.nuon", "{indent_width: 2}");
    let removed = dir.dir("removed");
    let filepath = dir.path("new/a.nu");
    let input_path = dir.path("stdin.input");
    fs::write(&input_path, BLOCK).unwrap();
    // The shell removes the directory it runs the binary in, which `Command` cannot.
    let output = Command::new("sh")
        .args([
            "-c",
            r#"cd "$1" && rmdir "$1" && exec "$0" format --stdin-filepath "$2" -"#,
        ])
        .arg(env!("CARGO_BIN_EXE_nuten"))
        .arg(&removed)
        .arg(&filepath)
        .stdin(fs::File::open(&input_path).unwrap())
        .output()
        .unwrap();
    assert_succeeds_quietly(&output);
    assert_eq!(stdout(&output), "if true {\n  ls\n}\n");
}

#[test]
fn format_takes_given_config_file_for_stdin() {
    let dir = TempDir::new();
    dir.file("nuten.nuon", "{indent_width: 3}");
    dir.file("two.nuon", "{indent_width: 2}");
    let output = format_stdin_in(&dir, &["-c", "two.nuon", "-"], BLOCK.as_bytes());
    assert_succeeds_quietly(&output);
    assert_eq!(stdout(&output), "if true {\n  ls\n}\n");
}

#[test]
fn format_writes_nothing_to_stdout_and_reports_syntax_error_of_stdin() {
    let dir = TempDir::new();
    let output = format_stdin_in(&dir, &["-"], b"ls\nls ||");
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    let problems = problems(&output);
    assert_eq!(problems.lines().count(), 1, "{problems}");
    assert!(problems.starts_with("<stdin>:2:4: "), "{problems}");
}

#[test]
fn format_reports_stdin_that_is_not_utf8() {
    let dir = TempDir::new();
    let output = format_stdin_in(&dir, &["-"], b"ls \xff");
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    let problems = problems(&output);
    assert!(problems.starts_with("<stdin>: "), "{problems}");
}

#[test]
fn format_writes_nothing_to_stdout_when_config_file_for_stdin_is_wrong() {
    let dir = TempDir::new();
    let nuon = dir.file("nuten.nuon", "{a: 1}");
    let output = format_stdin_in(&dir, &["-"], UNFORMATTED.as_bytes());
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    assert_eq!(
        problems(&output),
        format!("{}:1:2: unknown key `a`\n", nuon.display())
    );
}

#[test]
fn format_check_on_stdin_exits_with_1_and_shows_what_would_change() {
    let dir = TempDir::new();
    let unnamed = format_stdin_in(&dir, &["--check", "-"], UNFORMATTED.as_bytes());
    assert_eq!(unnamed.status.code(), Some(1), "{}", stderr(&unnamed));
    assert_eq!(
        stdout(&unnamed),
        format!(
            "1 file would be reformatted:\n\n{}",
            view_of_unformatted("<stdin>")
        )
    );
    assert!(unnamed.stderr.is_empty());
}

#[test]
fn format_check_on_stdin_shows_change_under_stdin_filepath() {
    let dir = TempDir::new();
    let args = ["--check", "--stdin-filepath", "a.nu", "-"];
    let output = format_stdin_in(&dir, &args, UNFORMATTED.as_bytes());
    assert_eq!(output.status.code(), Some(1), "{}", stderr(&output));
    assert_eq!(
        stdout(&output),
        format!(
            "1 file would be reformatted:\n\n{}",
            view_of_unformatted("a.nu")
        )
    );
}

#[test]
fn format_check_on_stdin_succeeds_quietly_when_it_is_formatted() {
    let dir = TempDir::new();
    let output = format_stdin_in(&dir, &["--check", "-"], FORMATTED.as_bytes());
    assert_succeeds_quietly(&output);
    assert!(output.stdout.is_empty());
}

#[test]
fn format_rejects_stdin_with_other_paths() {
    let dir = TempDir::new();
    let [path] = write_unformatted(&dir, ["a.nu"]);
    for args in [["-", "a.nu"], ["a.nu", "-"]] {
        let output = format_stdin_in(&dir, &args, UNFORMATTED.as_bytes());
        assert_eq!(output.status.code(), Some(2));
        assert!(output.stdout.is_empty());
        assert_eq!(
            problems(&output),
            "`-` must be the only path, since it reads stdin\n"
        );
        assert_eq!(read(&path), UNFORMATTED);
    }
}

#[test]
fn format_rejects_stdin_with_other_paths_before_reading_given_config_file() {
    let dir = TempDir::new();
    let args = ["-c", "missing.nuon", "-", "a.nu"];
    let output = format_stdin_in(&dir, &args, UNFORMATTED.as_bytes());
    assert_eq!(output.status.code(), Some(2));
    assert_eq!(
        problems(&output),
        "`-` must be the only path, since it reads stdin\n"
    );
}

#[test]
fn format_reads_stdin_in_full_before_reporting_config_file_of_it() {
    let dir = TempDir::new();
    dir.file("wrong.nuon", "{a: 1}");
    dir.file("d/nuten.nuon", "{a: 1}");
    // More than a pipe holds, so that writing fails if the binary exits before reading it all.
    let input = "ls\n".repeat(1 << 20);
    for (args, cwd) in [
        (&["-c", "wrong.nuon", "-"][..], dir.0.clone()),
        (&["-"], dir.path("d")),
    ] {
        let mut child = nuten_command()
            .arg("format")
            .args(args)
            .current_dir(&cwd)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        let mut stdin = child.stdin.take().unwrap();
        let input = input.clone();
        let writer = thread::spawn(move || io::Write::write_all(&mut stdin, input.as_bytes()));
        let output = child.wait_with_output().unwrap();
        assert!(writer.join().unwrap().is_ok(), "{args:?}");
        assert_eq!(output.status.code(), Some(2), "{args:?}");
        assert!(output.stdout.is_empty(), "{args:?}");
    }
}

#[test]
fn format_check_on_stdin_exits_with_2_on_syntax_error() {
    let dir = TempDir::new();
    let output = format_stdin_in(&dir, &["--check", "-"], b"ls ||");
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    let problems = problems(&output);
    assert!(problems.starts_with("<stdin>:1:4: "), "{problems}");
}

#[test]
fn format_counts_stdin_formatted_as_written_when_reader_closes_first() {
    let dir = TempDir::new();
    let (reader, writer) = io::pipe().unwrap();
    drop(reader);
    let output = format_stdin_command(&dir, &["-"], UNFORMATTED.as_bytes())
        .stdout(writer)
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    assert!(output.stderr.is_empty());
}

#[cfg(target_os = "linux")]
#[test]
fn format_reports_stdin_formatted_it_cannot_write() {
    let dir = TempDir::new();
    let full = fs::File::options().write(true).open("/dev/full").unwrap();
    let output = format_stdin_command(&dir, &["-"], UNFORMATTED.as_bytes())
        .stdout(full)
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    let problems = problems(&output);
    assert!(problems.starts_with("stdout: "), "{problems}");
}

#[test]
fn format_takes_config_file_nearest_stdin_filepath_though_file_does_not_exist() {
    let dir = TempDir::new();
    dir.file("nuten.nuon", "{indent_width: 3}");
    dir.file("d/nuten.nuon", "{indent_width: 2}");
    let args = ["--stdin-filepath", "d/new.nu", "-"];
    let output = format_stdin_in(&dir, &args, BLOCK.as_bytes());
    assert_succeeds_quietly(&output);
    assert_eq!(stdout(&output), "if true {\n  ls\n}\n");
    assert!(!dir.path("d/new.nu").exists());
}

#[test]
fn format_takes_config_file_nearest_existing_directory_of_stdin_filepath() {
    let dir = TempDir::new();
    dir.file("d/nuten.nuon", "{indent_width: 2}");
    for filepath in ["d/new/new/a.nu", "d/new/../a.nu"] {
        let args = ["--stdin-filepath", filepath, "-"];
        let output = format_stdin_in(&dir, &args, BLOCK.as_bytes());
        assert_succeeds_quietly(&output);
        assert_eq!(stdout(&output), "if true {\n  ls\n}\n", "{filepath}");
        assert!(!dir.path("d/new").exists());
    }
}

#[cfg(unix)]
#[test]
fn format_takes_config_file_where_symlinked_stdin_filepath_leads() {
    let dir = TempDir::new();
    dir.file("a/nuten.nuon", "{indent_width: 2}");
    dir.file("b/nuten.nuon", "{indent_width: 3}");
    let path = dir.file("a/a.nu", BLOCK);
    std::os::unix::fs::symlink(&path, dir.path("b/link.nu")).unwrap();
    let args = ["--stdin-filepath", "b/link.nu", "-"];
    let output = format_stdin_in(&dir, &args, BLOCK.as_bytes());
    assert_succeeds_quietly(&output);
    assert_eq!(stdout(&output), "if true {\n  ls\n}\n");
    assert_eq!(read(&path), BLOCK);
}

#[test]
fn format_takes_given_config_file_over_one_nearest_stdin_filepath() {
    let dir = TempDir::new();
    dir.file("d/nuten.nuon", "{indent_width: 3}");
    dir.file("two.nuon", "{indent_width: 2}");
    let args = ["-c", "two.nuon", "--stdin-filepath", "d/a.nu", "-"];
    let output = format_stdin_in(&dir, &args, BLOCK.as_bytes());
    assert_succeeds_quietly(&output);
    assert_eq!(stdout(&output), "if true {\n  ls\n}\n");
}

#[test]
fn format_reports_syntax_error_of_stdin_at_stdin_filepath() {
    let dir = TempDir::new();
    dir.dir("d");
    let args = ["--stdin-filepath", "d/a.nu", "-"];
    let output = format_stdin_in(&dir, &args, b"ls ||");
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    let problems = problems(&output);
    let expected = format!("{}:1:4: ", Path::new("d/a.nu").display());
    assert!(problems.starts_with(&expected), "{problems}");
}

#[test]
fn format_reports_stdin_that_is_not_utf8_at_stdin_filepath() {
    let dir = TempDir::new();
    let args = ["--stdin-filepath", "d/a.nu", "-"];
    let output = format_stdin_in(&dir, &args, b"ls \xff");
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    let problems = problems(&output);
    let expected = format!("{}: ", Path::new("d/a.nu").display());
    assert!(problems.starts_with(&expected), "{problems}");
}

#[test]
fn format_check_on_stdin_takes_config_file_nearest_stdin_filepath() {
    let dir = TempDir::new();
    dir.file("d/nuten.nuon", "{indent_width: 2}");
    let args = ["--check", "--stdin-filepath", "d/a.nu", "-"];
    let output = format_stdin_in(&dir, &args, b"if true {\n  ls\n}\n");
    assert_succeeds_quietly(&output);
    assert!(output.stdout.is_empty());
}

#[test]
fn format_formats_stdin_whose_filepath_ignore_file_ignores() {
    let dir = TempDir::new();
    dir.file(".ignore", "a.nu\n");
    let args = ["--stdin-filepath", "a.nu", "-"];
    let output = format_stdin_in(&dir, &args, UNFORMATTED.as_bytes());
    assert_succeeds_quietly(&output);
    assert_eq!(stdout(&output), FORMATTED);
}

#[test]
fn format_writes_stdin_as_it_is_when_exclude_matches_stdin_filepath() {
    let dir = TempDir::new();
    dir.file("nuten.nuon", r#"{exclude: ["vendor"]}"#);
    dir.dir("vendor");
    for filepath in ["vendor/a.nu", "vendor/new/a.nu"] {
        for input in [UNFORMATTED, "ls ||"] {
            let args = ["--stdin-filepath", filepath, "-"];
            let output = format_stdin_in(&dir, &args, input.as_bytes());
            assert_succeeds_quietly(&output);
            assert_eq!(stdout(&output), input, "{filepath}");
        }
    }
}

#[cfg(unix)]
#[test]
fn format_matches_stdin_filepath_where_symlinked_directory_leads() {
    let dir = TempDir::new();
    dir.file("nuten.nuon", r#"{exclude: ["vendor"]}"#);
    std::os::unix::fs::symlink(dir.dir("vendor"), dir.path("link")).unwrap();
    for filepath in ["link/a.nu", "link/new/a.nu"] {
        let args = ["--stdin-filepath", filepath, "-"];
        let output = format_stdin_in(&dir, &args, UNFORMATTED.as_bytes());
        assert_succeeds_quietly(&output);
        assert_eq!(stdout(&output), UNFORMATTED, "{filepath}");
    }
}

#[test]
fn format_check_on_stdin_succeeds_when_exclude_matches_stdin_filepath() {
    let dir = TempDir::new();
    dir.file("nuten.nuon", r#"{exclude: ["a.nu"]}"#);
    let args = ["--check", "--stdin-filepath", "a.nu", "-"];
    let output = format_stdin_in(&dir, &args, UNFORMATTED.as_bytes());
    assert_succeeds_quietly(&output);
    assert!(output.stdout.is_empty());
}

#[test]
fn format_matches_stdin_filepath_with_patterns_of_given_config_file_from_current_directory() {
    let dir = TempDir::new();
    dir.file("config/nuten.nuon", r#"{exclude: ["d/a.nu"]}"#);
    let args = ["-c", "config/nuten.nuon", "--stdin-filepath", "d/a.nu", "-"];
    let output = format_stdin_in(&dir, &args, UNFORMATTED.as_bytes());
    assert_succeeds_quietly(&output);
    assert_eq!(stdout(&output), UNFORMATTED);
}

#[test]
fn format_formats_stdin_whose_filepath_include_misses() {
    let dir = TempDir::new();
    dir.file("nuten.nuon", r#"{include: ["*.nush"]}"#);
    let args = ["--stdin-filepath", "a.nu", "-"];
    let output = format_stdin_in(&dir, &args, UNFORMATTED.as_bytes());
    assert_succeeds_quietly(&output);
    assert_eq!(stdout(&output), FORMATTED);
}

#[test]
fn format_rejects_stdin_filepath_without_stdin() {
    let dir = TempDir::new();
    let [path] = write_unformatted(&dir, ["a.nu"]);
    for args in [
        &["--stdin-filepath", "a.nu", "a.nu"][..],
        &["--stdin-filepath", "a.nu"],
    ] {
        let output = format_in(&dir, args);
        assert_eq!(output.status.code(), Some(2));
        assert_eq!(
            problems(&output),
            "`--stdin-filepath` needs `-`, which reads stdin\n"
        );
        assert_eq!(read(&path), UNFORMATTED);
    }
}

#[test]
fn format_rejects_stdin_filepath_of_no_file() {
    let dir = TempDir::new();
    dir.dir("d");
    for filepath in ["", "d", "d/..", "/"] {
        let args = ["--stdin-filepath", filepath, "-"];
        let output = format_stdin_in(&dir, &args, UNFORMATTED.as_bytes());
        assert_eq!(output.status.code(), Some(2), "{filepath:?}");
        assert!(output.stdout.is_empty(), "{filepath:?}");
        let expected = format!("`--stdin-filepath` needs the path of a file, not `{filepath}`\n");
        assert_eq!(problems(&output), expected);
    }
}

#[test]
fn format_rejects_stdin_filepath_before_reading_given_config_file() {
    let dir = TempDir::new();
    dir.dir("d");
    let args = ["-c", "missing.nuon", "--stdin-filepath", "a.nu", "a.nu"];
    let output = format_in(&dir, &args);
    assert_eq!(
        problems(&output),
        "`--stdin-filepath` needs `-`, which reads stdin\n"
    );
    let args = ["-c", "missing.nuon", "--stdin-filepath", "d", "-"];
    let output = format_stdin_in(&dir, &args, UNFORMATTED.as_bytes());
    assert_eq!(
        problems(&output),
        "`--stdin-filepath` needs the path of a file, not `d`\n"
    );
}

#[test]
fn format_takes_config_file_where_stdin_filepath_leads_through_dot_dot_after_missing_directory() {
    let dir = TempDir::new();
    dir.file("nuten.nuon", "{indent_width: 3}");
    dir.file("ok/nuten.nuon", "{indent_width: 2}");
    dir.file("wrong/nuten.nuon", "{a: 1}");
    for (filepath, expected) in [
        ("ok/nope/../../q.nu", "if true {\n   ls\n}\n"),
        ("wrong/nope/../../ok/q.nu", "if true {\n  ls\n}\n"),
        ("nope/x/../../ok/q.nu", "if true {\n  ls\n}\n"),
    ] {
        let args = ["--stdin-filepath", filepath, "-"];
        let output = format_stdin_in(&dir, &args, BLOCK.as_bytes());
        assert_succeeds_quietly(&output);
        assert_eq!(stdout(&output), expected, "{filepath}");
    }
}

#[cfg(unix)]
#[test]
fn format_takes_config_file_above_symlink_target_after_stdin_filepath_leaves_missing_directory() {
    let dir = TempDir::new();
    dir.file("nuten.nuon", "{indent_width: 3}");
    dir.file("x/nuten.nuon", "{indent_width: 2}");
    dir.file("x/y/nuten.nuon", "{indent_width: 5}");
    std::os::unix::fs::symlink(dir.path("x/y"), dir.path("link")).unwrap();
    for filepath in ["nope/../link/../q.nu", "nope/../link/sub/../../q.nu"] {
        let args = ["--stdin-filepath", filepath, "-"];
        let output = format_stdin_in(&dir, &args, BLOCK.as_bytes());
        assert_succeeds_quietly(&output);
        assert_eq!(stdout(&output), "if true {\n  ls\n}\n", "{filepath}");
    }
}

#[cfg(unix)]
#[test]
fn format_takes_config_file_beside_symlink_to_root_after_stdin_filepath_leaves_missing_directory() {
    let dir = TempDir::new();
    dir.file("nuten.nuon", "{indent_width: 2}");
    std::os::unix::fs::symlink("/", dir.path("rootlink")).unwrap();
    let args = ["--stdin-filepath", "nope/../rootlink", "-"];
    let output = format_stdin_in(&dir, &args, BLOCK.as_bytes());
    assert_succeeds_quietly(&output);
    assert_eq!(stdout(&output), "if true {\n  ls\n}\n");
}

#[test]
fn format_takes_config_file_nearest_stdin_filepath_that_leaves_current_directory() {
    let dir = TempDir::new();
    dir.file("d/nuten.nuon", "{indent_width: 2}");
    let name = dir.0.file_name().unwrap().to_str().unwrap();
    let filepath = format!("../{name}/d/new.nu");
    let args = ["--stdin-filepath", filepath.as_str(), "-"];
    let output = format_stdin_in(&dir, &args, BLOCK.as_bytes());
    assert_succeeds_quietly(&output);
    assert_eq!(stdout(&output), "if true {\n  ls\n}\n");
}
