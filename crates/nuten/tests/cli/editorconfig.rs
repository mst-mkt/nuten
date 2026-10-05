use crate::support::{
    BLOCK, TempDir, assert_succeeds_quietly, format, format_in, format_stdin_in, problems, read,
    stderr, stdout,
};

#[test]
fn format_takes_editorconfig_in_ancestor_directory() {
    let dir = TempDir::new();
    dir.file(".editorconfig", "[*]\nindent_size = 2\n");
    let path = dir.file("a/b/a.nu", BLOCK);
    let output = format(&[&path]);
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    assert_eq!(read(&path), "if true {\n  ls\n}\n");
}

#[test]
fn format_takes_only_editorconfig_sections_matching_file() {
    let dir = TempDir::new();
    dir.file(
        ".editorconfig",
        "[*.sh]\nindent_size = 3\n[d/*.nu]\nindent_size = 2\n",
    );
    let matched = dir.file("d/a.nu", BLOCK);
    let unmatched = dir.file("e/d/a.nu", BLOCK);
    let output = format(&[&matched, &unmatched]);
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    assert_eq!(read(&matched), "if true {\n  ls\n}\n");
    assert_eq!(read(&unmatched), "if true {\n    ls\n}\n");
}

#[test]
fn format_takes_nearer_editorconfig_over_farther_one() {
    let dir = TempDir::new();
    dir.file(
        ".editorconfig",
        "[*]\nindent_size = 2\nindent_style = tab\n",
    );
    dir.file("d/.editorconfig", "[*]\nindent_style = space\n");
    let path = dir.file("d/a.nu", BLOCK);
    let output = format(&[&path]);
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    assert_eq!(read(&path), "if true {\n  ls\n}\n");
}

#[test]
fn format_takes_settings_of_config_file_over_editorconfig() {
    let dir = TempDir::new();
    dir.file(
        ".editorconfig",
        "[*]\nindent_size = 2\nindent_style = tab\n",
    );
    dir.file("nuten.nuon", "{indent_style: space}");
    let path = dir.file("a.nu", BLOCK);
    let output = format(&[&path]);
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    assert_eq!(read(&path), "if true {\n  ls\n}\n");
}

#[test]
fn format_takes_overrides_of_config_file_over_editorconfig() {
    let dir = TempDir::new();
    dir.file(".editorconfig", "[*]\nindent_size = 2\n");
    dir.file(
        "nuten.nuon",
        "{overrides: [{include: [b.nu], indent_width: 3}]}",
    );
    let [a, b] = ["a.nu", "b.nu"].map(|name| dir.file(name, BLOCK));
    let output = format(&[&a, &b]);
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    assert_eq!(read(&a), "if true {\n  ls\n}\n");
    assert_eq!(read(&b), "if true {\n   ls\n}\n");
}

#[test]
fn format_leaves_editorconfig_unread_when_config_file_turns_it_off() {
    let dir = TempDir::new();
    dir.file(".editorconfig", "[*]\nindent_size = 2\n");
    dir.file("nuten.nuon", "{editorconfig: false}");
    let path = dir.file("a.nu", BLOCK);
    let output = format(&[&path]);
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    assert_eq!(read(&path), "if true {\n    ls\n}\n");
}

#[test]
fn format_takes_no_editorconfig_above_one_with_root() {
    let dir = TempDir::new();
    dir.file(
        ".editorconfig",
        "[*]\nindent_size = 2\nindent_style = tab\n",
    );
    dir.file("d/.editorconfig", "root = true\n[*]\nindent_size = 3\n");
    let path = dir.file("d/a.nu", BLOCK);
    let output = format(&[&path]);
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    assert_eq!(read(&path), "if true {\n   ls\n}\n");
}

#[test]
fn format_takes_no_editorconfig_above_repository_root() {
    let dir = TempDir::new();
    dir.file(
        ".editorconfig",
        "[*]\nindent_size = 2\nindent_style = tab\n",
    );
    dir.dir("d/.git");
    dir.file("d/.editorconfig", "[*]\nindent_size = 3\n");
    let path = dir.file("d/a.nu", BLOCK);
    let output = format(&[&path]);
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    assert_eq!(read(&path), "if true {\n   ls\n}\n");
}

#[test]
fn format_takes_editorconfig_of_each_file_in_walked_directory() {
    let dir = TempDir::new();
    dir.file(".editorconfig", "[*]\nindent_size = 2\n");
    dir.file("d/.editorconfig", "[*]\nindent_size = 3\n");
    let top = dir.file("a.nu", BLOCK);
    let inner = dir.file("d/a.nu", BLOCK);
    let output = format_in(&dir, &["."]);
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    assert_eq!(read(&top), "if true {\n  ls\n}\n");
    assert_eq!(read(&inner), "if true {\n   ls\n}\n");
}

#[test]
fn format_takes_editorconfig_with_given_config_file() {
    let dir = TempDir::new();
    dir.file(
        ".editorconfig",
        "[*]\nindent_size = 2\nindent_style = tab\n",
    );
    dir.file("config/nuten.nuon", "{indent_style: space}");
    let path = dir.file("a.nu", BLOCK);
    let output = format_in(&dir, &["-c", "config/nuten.nuon", "a.nu"]);
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    assert_eq!(read(&path), "if true {\n  ls\n}\n");
}

#[test]
fn format_takes_editorconfig_for_stdin_filepath_though_file_does_not_exist() {
    let dir = TempDir::new();
    dir.file(".editorconfig", "[d/new/*.nu]\nindent_size = 2\n");
    let args = ["--stdin-filepath", "d/new/a.nu", "-"];
    let output = format_stdin_in(&dir, &args, BLOCK.as_bytes());
    assert_succeeds_quietly(&output);
    assert_eq!(stdout(&output), "if true {\n  ls\n}\n");
}

#[test]
fn format_takes_no_editorconfig_for_stdin_without_filepath() {
    let dir = TempDir::new();
    dir.file(".editorconfig", "[*]\nindent_size = 2\n");
    let output = format_stdin_in(&dir, &["-"], BLOCK.as_bytes());
    assert_succeeds_quietly(&output);
    assert_eq!(stdout(&output), "if true {\n    ls\n}\n");
}

#[test]
fn format_takes_editorconfig_for_stdin_filepath_with_given_config_file() {
    let dir = TempDir::new();
    dir.file(".editorconfig", "[*]\nindent_size = 2\n");
    dir.file("config/nuten.nuon", "{}");
    let args = ["-c", "config/nuten.nuon", "--stdin-filepath", "a.nu", "-"];
    let output = format_stdin_in(&dir, &args, BLOCK.as_bytes());
    assert_succeeds_quietly(&output);
    assert_eq!(stdout(&output), "if true {\n  ls\n}\n");
}

#[test]
fn format_takes_given_config_file_and_editorconfig_for_existing_stdin_filepath() {
    let dir = TempDir::new();
    dir.file(".editorconfig", "[*]\nindent_size = 2\n");
    dir.file("nuten.nuon", "{indent_width: 3}");
    dir.file("config/nuten.nuon", "{}");
    dir.file("a.nu", "");
    let args = ["-c", "config/nuten.nuon", "--stdin-filepath", "a.nu", "-"];
    let output = format_stdin_in(&dir, &args, BLOCK.as_bytes());
    assert_succeeds_quietly(&output);
    assert_eq!(stdout(&output), "if true {\n  ls\n}\n");
}

#[test]
fn format_leaves_editorconfig_unread_when_given_config_file_turns_it_off() {
    let dir = TempDir::new();
    dir.file(".editorconfig", "[*]\nindent_size = 2\n");
    dir.file("config/nuten.nuon", "{editorconfig: false}");
    let path = dir.file("a.nu", BLOCK);
    let output = format_in(&dir, &["-c", "config/nuten.nuon", "a.nu"]);
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    assert_eq!(read(&path), "if true {\n    ls\n}\n");
}

#[test]
fn format_takes_indent_size_of_editorconfig_when_config_file_indents_with_spaces() {
    let dir = TempDir::new();
    dir.file(
        ".editorconfig",
        "[*]\nindent_style = tab\nindent_size = 2\ntab_width = 8\n",
    );
    dir.file("nuten.nuon", "{indent_style: space}");
    let path = dir.file("a.nu", BLOCK);
    let output = format(&[&path]);
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    assert_eq!(read(&path), "if true {\n  ls\n}\n");
}

#[test]
fn format_takes_tab_width_of_editorconfig_when_config_file_indents_with_tabs() {
    let dir = TempDir::new();
    dir.file(
        ".editorconfig",
        "[*]\nindent_size = 2\ntab_width = 3\nmax_line_length = 10\n",
    );
    dir.file("nuten.nuon", "{indent_style: tab}");
    let path = dir.file("a.nu", "if true {\n[aa bb]\n}");
    let output = format(&[&path]);
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    assert_eq!(read(&path), "if true {\n\t[\n\t\taa\n\t\tbb\n\t]\n}\n");
}

#[test]
fn format_takes_tab_width_of_editorconfig_when_override_indents_with_tabs() {
    let dir = TempDir::new();
    dir.file(
        ".editorconfig",
        "[*]\nindent_size = 2\ntab_width = 3\nmax_line_length = 10\n",
    );
    dir.file(
        "nuten.nuon",
        "{overrides: [{include: [a.nu], indent_style: tab}]}",
    );
    let path = dir.file("a.nu", "if true {\n[aa bb]\n}");
    let output = format(&[&path]);
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    assert_eq!(read(&path), "if true {\n\t[\n\t\taa\n\t\tbb\n\t]\n}\n");
}

#[test]
fn format_breaks_line_longer_than_max_line_length_of_editorconfig() {
    let dir = TempDir::new();
    dir.file(".editorconfig", "[*]\nmax_line_length = 10\n");
    let path = dir.file("a.nu", "[aaaa bbbb cccc]");
    let output = format(&[&path]);
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    assert_eq!(read(&path), "[\n    aaaa\n    bbbb\n    cccc\n]\n");
}

#[test]
fn format_keeps_long_line_when_max_line_length_of_editorconfig_is_off() {
    let dir = TempDir::new();
    dir.file(".editorconfig", "[*]\nmax_line_length = off\n");
    let line = format!("[{}]\n", vec!["aaaa"; 40].join(", "));
    let path = dir.file("a.nu", &line);
    let output = format(&[&path]);
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    assert_eq!(read(&path), line);
}

#[cfg(unix)]
#[test]
fn format_takes_editorconfig_where_symlinked_file_leads() {
    let dir = TempDir::new();
    dir.file("real/.editorconfig", "[*]\nindent_size = 3\n");
    dir.file("other/.editorconfig", "[link.nu]\nindent_size = 2\n");
    let path = dir.file("real/a.nu", BLOCK);
    let link = dir.path("other/link.nu");
    std::os::unix::fs::symlink(&path, &link).unwrap();
    let output = format(&[&link]);
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    assert_eq!(read(&path), "if true {\n   ls\n}\n");
}

#[test]
fn format_writes_nothing_to_stdout_when_editorconfig_of_stdin_filepath_cannot_be_read() {
    let dir = TempDir::new();
    let editorconfig = dir.dir("d/.editorconfig");
    dir.file("d/a.nu", BLOCK);
    let args = ["--stdin-filepath", "d/a.nu", "-"];
    let output = format_stdin_in(&dir, &args, BLOCK.as_bytes());
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    assert_eq!(
        problems(&output),
        format!("{}: not a regular file\n", editorconfig.display())
    );
}

#[test]
fn format_writes_nothing_to_stdout_when_editorconfig_of_unwritten_stdin_filepath_cannot_be_read() {
    let dir = TempDir::new();
    let editorconfig = dir.dir("d/.editorconfig");
    let args = ["--stdin-filepath", "d/new/a.nu", "-"];
    let output = format_stdin_in(&dir, &args, BLOCK.as_bytes());
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    assert_eq!(
        problems(&output),
        format!("{}: not a regular file\n", editorconfig.display())
    );
}

#[test]
fn format_writes_nothing_when_any_editorconfig_cannot_be_read() {
    let dir = TempDir::new();
    let editorconfig = dir.dir("d/.editorconfig");
    let path = dir.file("d/a.nu", BLOCK);
    let other = dir.file("e/a.nu", BLOCK);
    let output = format(&[&path, &other]);
    assert_eq!(output.status.code(), Some(2));
    assert_eq!(read(&path), BLOCK);
    assert_eq!(read(&other), BLOCK);
    assert_eq!(
        problems(&output),
        format!("{}: not a regular file\n", editorconfig.display())
    );
}
