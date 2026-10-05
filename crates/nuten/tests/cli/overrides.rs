use crate::support::{
    BLOCK, TempDir, assert_succeeds, assert_succeeds_quietly, format_in, format_stdin_in,
    nuten_command, read, stdout,
};

#[test]
fn format_takes_overrides_for_files_they_pick_under_directory_and_given_directly() {
    let dir = TempDir::new();
    dir.file(
        "nuten.nuon",
        r#"{overrides: [
            {include: ["two/**"], exclude: ["eight"], indent_width: 2}
            {include: ["eight/*.nu"], indent_width: 8}
            {include: ["two/b.nu"], indent_width: 3}
        ]}"#,
    );
    let paths = [
        "two/a.nu",
        "two/b.nu",
        "two/eight/a.nu",
        "eight/a.nu",
        "a.nu",
    ]
    .map(|name| dir.file(name, BLOCK));
    let output = format_in(&dir, &["two", "eight/a.nu", "a.nu"]);
    assert_succeeds(&output);
    let indents = paths.map(|path| read(&path).lines().nth(1).unwrap().len() - "ls".len());
    assert_eq!(indents, [2, 3, 4, 8, 4]);
}

#[test]
fn format_takes_overrides_of_given_config_file_from_current_directory() {
    let dir = TempDir::new();
    dir.file(
        "config/nuten.nuon",
        r#"{overrides: [{include: ["d/*.nu"], indent_width: 2}]}"#,
    );
    let path = dir.file("d/a.nu", BLOCK);
    let output = format_in(&dir, &["-c", "config/nuten.nuon", "d"]);
    assert_succeeds(&output);
    assert_eq!(read(&path), "if true {\n  ls\n}\n");
}

#[test]
fn format_takes_overrides_for_stdin_filepath() {
    let dir = TempDir::new();
    dir.file(
        "nuten.nuon",
        r#"{overrides: [{include: ["d/*.nu"], indent_width: 2}]}"#,
    );
    for (filepath, formatted) in [
        ("d/a.nu", "if true {\n  ls\n}\n"),
        ("e/a.nu", "if true {\n    ls\n}\n"),
    ] {
        let args = ["--stdin-filepath", filepath, "-"];
        let output = format_stdin_in(&dir, &args, BLOCK.as_bytes());
        assert_succeeds_quietly(&output);
        assert_eq!(stdout(&output), formatted, "{filepath}");
    }
}

#[test]
fn format_takes_no_overrides_for_stdin_without_filepath() {
    let dir = TempDir::new();
    dir.file(
        "nuten.nuon",
        r#"{overrides: [{include: ["*"], indent_width: 2}]}"#,
    );
    let output = format_stdin_in(&dir, &["-"], BLOCK.as_bytes());
    assert_succeeds_quietly(&output);
    assert_eq!(stdout(&output), "if true {\n    ls\n}\n");
}

#[test]
fn format_takes_overrides_of_given_config_file_for_stdin_filepath() {
    let dir = TempDir::new();
    dir.file(
        "config/nuten.nuon",
        r#"{overrides: [{include: ["d/*.nu"], indent_width: 2}]}"#,
    );
    let args = ["-c", "config/nuten.nuon", "--stdin-filepath", "d/a.nu", "-"];
    let output = format_stdin_in(&dir, &args, BLOCK.as_bytes());
    assert_succeeds_quietly(&output);
    assert_eq!(stdout(&output), "if true {\n  ls\n}\n");
}

#[test]
fn format_takes_only_overrides_without_slash_outside_current_directory_of_given_config_file() {
    let dir = TempDir::new();
    dir.file(
        "nuten.nuon",
        r#"{overrides: [
            {include: ["a.nu"], indent_width: 2}
            {include: ["d/b.nu"], indent_width: 8}
        ]}"#,
    );
    let [a, b] = ["d/a.nu", "d/b.nu"].map(|name| dir.file(name, BLOCK));
    let output = nuten_command()
        .args(["format", "-c", "../nuten.nuon", "../d"])
        .current_dir(dir.dir("e"))
        .output()
        .unwrap();
    assert_succeeds(&output);
    assert_eq!(read(&a), "if true {\n  ls\n}\n");
    assert_eq!(read(&b), "if true {\n    ls\n}\n");
}
