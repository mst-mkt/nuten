use crate::support::{
    FORMATTED, TempDir, UNFORMATTED, format_in, format_stdin_in, nuten_command, stderr,
};

#[test]
fn format_ends_with_summary_of_files_it_formats_and_leaves_unchanged() {
    let dir = TempDir::new();
    dir.file("a.nu", UNFORMATTED);
    dir.file("b.nu", FORMATTED);
    let output = format_in(&dir, &["a.nu", "b.nu"]);
    assert_eq!(output.status.code(), Some(0));
    assert_eq!(stderr(&output), "2 files: 1 formatted, 1 unchanged\n");
}

#[test]
fn format_check_ends_with_summary_of_files_that_need_formatting() {
    let dir = TempDir::new();
    dir.file("a.nu", UNFORMATTED);
    dir.file("b.nu", UNFORMATTED);
    dir.file("c.nu", FORMATTED);
    let output = format_in(&dir, &["--check", "a.nu", "b.nu", "c.nu"]);
    assert_eq!(output.status.code(), Some(1));
    assert_eq!(
        stderr(&output),
        "3 files: 2 need formatting (2 lines, 2 whitespace only), 1 unchanged\n"
    );
    let one = format_in(&dir, &["--check", "a.nu"]);
    assert_eq!(
        stderr(&one),
        "1 file: 1 needs formatting (1 line, 1 whitespace only)\n"
    );
}

#[test]
fn format_ends_with_summary_after_problems() {
    let dir = TempDir::new();
    dir.file("a.nu", UNFORMATTED);
    dir.file("bad.nu", "ls || x\n");
    let output = format_in(&dir, &["a.nu", "bad.nu"]);
    assert_eq!(output.status.code(), Some(2));
    let stderr = stderr(&output);
    assert!(stderr.starts_with("error: bad.nu:1:4: "), "{stderr}");
    assert!(
        stderr.ends_with("\n2 files: 1 formatted, 1 failed\n"),
        "{stderr}"
    );
}

#[test]
fn format_counts_files_it_leaves_alone_for_wrong_config_file_as_skipped() {
    let dir = TempDir::new();
    dir.file("d/nuten.nuon", "{include: 1}");
    dir.file("d/a.nu", UNFORMATTED);
    dir.file("b.nu", UNFORMATTED);
    let output = format_in(&dir, &["d/a.nu", "b.nu"]);
    assert_eq!(output.status.code(), Some(2));
    let stderr = stderr(&output);
    assert!(stderr.ends_with("\n2 files: 2 skipped\n"), "{stderr}");
}

#[test]
fn format_ends_with_no_summary_for_stdin() {
    let dir = TempDir::new();
    let output = format_stdin_in(&dir, &["-"], b"ls || x\n");
    assert_eq!(output.status.code(), Some(2));
    let stderr = stderr(&output);
    assert_eq!(stderr.lines().count(), 1, "{stderr}");
    assert!(stderr.starts_with("error: <stdin>:1:4: "), "{stderr}");
}

#[test]
fn format_ends_with_no_summary_when_arguments_are_wrong() {
    let dir = TempDir::new();
    dir.file("a.nu", UNFORMATTED);
    let output = format_in(&dir, &["--stdin-filepath", "a.nu", "a.nu"]);
    assert_eq!(output.status.code(), Some(2));
    assert_eq!(
        stderr(&output),
        "error: `--stdin-filepath` needs `-`, which reads stdin\n"
    );
}

#[test]
fn format_puts_counts_of_summary_in_bold_when_clicolor_force_is_set() {
    let dir = TempDir::new();
    dir.file("a.nu", UNFORMATTED);
    let output = nuten_command()
        .args(["format", "a.nu"])
        .current_dir(&dir.0)
        .env("CLICOLOR_FORCE", "1")
        .output()
        .unwrap();
    assert_eq!(
        stderr(&output),
        "\u{1b}[1m1\u{1b}[0m file: \u{1b}[1m1\u{1b}[0m formatted\n"
    );
}

#[cfg(target_os = "linux")]
#[test]
fn format_succeeds_though_it_cannot_write_summary() {
    let dir = TempDir::new();
    dir.file("a.nu", UNFORMATTED);
    let full = std::fs::File::options()
        .write(true)
        .open("/dev/full")
        .unwrap();
    let output = nuten_command()
        .args(["format", "a.nu"])
        .current_dir(&dir.0)
        .stderr(full)
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(0));
}
