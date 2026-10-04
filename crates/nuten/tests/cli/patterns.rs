#[cfg(unix)]
use std::fs;
use std::path::PathBuf;

use crate::support::{
    FORMATTED, TempDir, UNFORMATTED, assert_succeeds_quietly, format_in, nuten_command, read,
    stderr, write_unformatted,
};

fn assert_formatted_only(formatted: &[PathBuf], unformatted: &[PathBuf]) {
    for path in formatted {
        assert_eq!(read(path), FORMATTED, "{}", path.display());
    }
    for path in unformatted {
        assert_eq!(read(path), UNFORMATTED, "{}", path.display());
    }
}

#[test]
fn format_takes_files_include_matches_under_directory_in_place_of_nu_files() {
    let dir = TempDir::new();
    dir.file("d/nuten.nuon", r#"{include: ["*.nush", "bin/*"]}"#);
    let taken = write_unformatted(&dir, ["d/a.nush", "d/e/b.nush", "d/bin/run"]);
    let skipped = write_unformatted(&dir, ["d/a.nu", "d/e/bin/run", "d/bin/sub/run"]);
    let output = format_in(&dir, &["d"]);
    assert_succeeds_quietly(&output);
    assert_formatted_only(&taken, &skipped);
}

#[test]
fn format_skips_files_exclude_matches_under_directory() {
    let dir = TempDir::new();
    dir.file(
        "d/nuten.nuon",
        r#"{exclude: ["vendor", "gen/*.nu", "*.gen.nu"]}"#,
    );
    let skipped = write_unformatted(
        &dir,
        [
            "d/vendor/a.nu",
            "d/e/vendor/f/a.nu",
            "d/gen/a.nu",
            "d/e/a.gen.nu",
        ],
    );
    let taken = write_unformatted(&dir, ["d/a.nu", "d/e/gen/a.nu", "d/gen/e/a.nu"]);
    let output = format_in(&dir, &["d"]);
    assert_succeeds_quietly(&output);
    assert_formatted_only(&taken, &skipped);
}

#[test]
fn format_skips_file_both_include_and_exclude_match() {
    let dir = TempDir::new();
    dir.file("nuten.nuon", r#"{include: ["*.nu"], exclude: ["a.nu"]}"#);
    let [skipped, taken] = write_unformatted(&dir, ["a.nu", "b.nu"]);
    let output = format_in(&dir, &["."]);
    assert_succeeds_quietly(&output);
    assert_formatted_only(&[taken], &[skipped]);
}

#[test]
fn format_matches_patterns_from_directory_of_config_file() {
    let dir = TempDir::new();
    dir.file("d/nuten.nuon", r#"{exclude: ["/a.nu", "./e/b.nu", "f/"]}"#);
    let skipped = write_unformatted(&dir, ["d/a.nu", "d/e/b.nu", "d/e/f/a.nu"]);
    let taken = write_unformatted(&dir, ["d/e/a.nu", "d/b.nu"]);
    let output = format_in(&dir, &["d/e", "d/a.nu", "d/b.nu"]);
    assert_succeeds_quietly(&output);
    assert_formatted_only(&taken, &skipped);
}

#[test]
fn format_takes_include_and_exclude_from_config_file_nearest_each_file() {
    let dir = TempDir::new();
    dir.file("d/nuten.nuon", r#"{exclude: ["a.nu"]}"#);
    dir.file("d/e/nuten.nuon", "{}");
    dir.file("d/f/nuten.nuon", r#"{include: ["*.nush"]}"#);
    let skipped = write_unformatted(&dir, ["d/a.nu", "d/f/b.nu"]);
    let taken = write_unformatted(&dir, ["d/b.nu", "d/e/a.nu", "d/f/a.nush"]);
    let output = format_in(&dir, &["d"]);
    assert_succeeds_quietly(&output);
    assert_formatted_only(&taken, &skipped);
}

#[test]
fn format_matches_patterns_of_given_config_file_from_current_directory() {
    let dir = TempDir::new();
    dir.file("config/nuten.nuon", r#"{exclude: ["d/a.nu"]}"#);
    let [skipped, taken] = write_unformatted(&dir, ["d/a.nu", "d/e/d/a.nu"]);
    let output = format_in(&dir, &["-c", "config/nuten.nuon", "d"]);
    assert_succeeds_quietly(&output);
    assert_formatted_only(&[taken], &[skipped]);
}

#[test]
fn format_matches_only_patterns_without_slash_outside_current_directory_of_given_config_file() {
    let dir = TempDir::new();
    dir.file("nuten.nuon", r#"{exclude: ["a.nu", "d/b.nu"]}"#);
    let [skipped, taken] = write_unformatted(&dir, ["d/a.nu", "d/b.nu"]);
    let output = nuten_command()
        .args(["format", "-c", "../nuten.nuon", "."])
        .current_dir(dir.dir("e"))
        .output()
        .unwrap();
    assert_succeeds_quietly(&output);
    let output = nuten_command()
        .args(["format", "-c", "../nuten.nuon", "../d"])
        .current_dir(dir.path("e"))
        .output()
        .unwrap();
    assert_succeeds_quietly(&output);
    assert_formatted_only(&[taken], &[skipped]);
}

#[test]
fn format_formats_file_given_directly_though_include_misses_it() {
    let dir = TempDir::new();
    dir.file("nuten.nuon", r#"{include: ["*.nush"]}"#);
    let [path] = write_unformatted(&dir, ["a.nu"]);
    let output = format_in(&dir, &["a.nu"]);
    assert_succeeds_quietly(&output);
    assert_eq!(read(&path), FORMATTED);
}

const ALL_EXCLUDED: &str = "nothing to format, since `exclude` leaves out every file\n";

#[test]
fn format_leaves_file_given_directly_alone_when_exclude_matches() {
    let dir = TempDir::new();
    dir.file("nuten.nuon", r#"{exclude: ["vendor"]}"#);
    let [skipped, taken] = write_unformatted(&dir, ["vendor/a.nu", "b.nu"]);
    let output = format_in(&dir, &["vendor/a.nu", "b.nu"]);
    assert_succeeds_quietly(&output);
    assert_formatted_only(&[taken], &[skipped]);
}

#[test]
fn format_says_so_and_succeeds_when_exclude_leaves_out_every_file() {
    let dir = TempDir::new();
    dir.file("nuten.nuon", r#"{exclude: ["vendor"]}"#);
    let unformatted = dir.file("vendor/a.nu", UNFORMATTED);
    let broken = dir.file("vendor/b.nu", "ls ||");
    for args in [
        &["vendor/a.nu", "vendor/b.nu"][..],
        &["vendor"],
        &["--check", "vendor"],
    ] {
        let output = format_in(&dir, args);
        assert_eq!(output.status.code(), Some(0), "{args:?}");
        assert_eq!(stderr(&output), ALL_EXCLUDED, "{args:?}");
        assert!(output.stdout.is_empty(), "{args:?}");
    }
    assert_eq!(read(&unformatted), UNFORMATTED);
    assert_eq!(read(&broken), "ls ||");
}

#[test]
fn format_succeeds_quietly_when_exclude_matches_only_files_include_skips() {
    let dir = TempDir::new();
    dir.file("nuten.nuon", r#"{exclude: ["vendor"]}"#);
    dir.file("vendor/a.txt", "");
    let output = format_in(&dir, &["."]);
    assert_succeeds_quietly(&output);
}

#[test]
fn format_matches_exclude_without_slash_of_given_config_file_with_directories() {
    let dir = TempDir::new();
    dir.file("config/nuten.nuon", r#"{exclude: ["vendor"]}"#);
    for args in [
        &["-c", "config/nuten.nuon", "."][..],
        &["-c", "config/nuten.nuon", "vendor/e/a.nu", "a.nu"],
    ] {
        let [skipped, taken] = write_unformatted(&dir, ["vendor/e/a.nu", "a.nu"]);
        let output = format_in(&dir, args);
        assert_succeeds_quietly(&output);
        assert_formatted_only(&[taken], &[skipped]);
    }
}

#[cfg(unix)]
#[test]
fn format_matches_patterns_where_symlinked_directory_given_leads() {
    let dir = TempDir::new();
    dir.file("nuten.nuon", r#"{exclude: ["/real/b.nu"]}"#);
    let [taken, skipped] = write_unformatted(&dir, ["real/a.nu", "real/b.nu"]);
    std::os::unix::fs::symlink(dir.path("real"), dir.path("link")).unwrap();
    let output = format_in(&dir, &["link"]);
    assert_succeeds_quietly(&output);
    assert_formatted_only(&[taken], &[skipped]);
}

#[cfg(unix)]
#[test]
fn format_leaves_symlink_given_directly_alone_when_exclude_matches_where_it_leads() {
    let dir = TempDir::new();
    dir.file("nuten.nuon", r#"{exclude: ["vendor"]}"#);
    let [path] = write_unformatted(&dir, ["vendor/a.nu"]);
    std::os::unix::fs::symlink(&path, dir.path("link.nu")).unwrap();
    let output = format_in(&dir, &["link.nu"]);
    assert_eq!(output.status.code(), Some(0));
    assert_eq!(stderr(&output), ALL_EXCLUDED);
    assert_eq!(read(&path), UNFORMATTED);
}

#[cfg(unix)]
#[test]
fn format_leaves_file_alone_under_every_name_when_exclude_matches_one() {
    let dir = TempDir::new();
    // The config file excludes itself, so that `.` holds nothing but excluded files.
    dir.file("nuten.nuon", "{ exclude: [vendor, \"nuten.nuon\"] }\n");
    let [path] = write_unformatted(&dir, ["vendor/a.nu"]);
    fs::hard_link(&path, dir.path("b.nu")).unwrap();
    for args in [
        &["vendor/a.nu", "b.nu"][..],
        &["b.nu", "vendor/a.nu"],
        &["."],
    ] {
        let output = format_in(&dir, args);
        assert_eq!(output.status.code(), Some(0), "{args:?}");
        assert_eq!(stderr(&output), ALL_EXCLUDED, "{args:?}");
        assert_eq!(read(&path), UNFORMATTED, "{args:?}");
    }
}

#[test]
fn format_writes_nothing_when_config_file_nearest_file_include_misses_is_wrong() {
    let dir = TempDir::new();
    let nuon = dir.file("d/e/nuten.nuon", "{include: 1}");
    dir.file("d/e/a.txt", "");
    let [path] = write_unformatted(&dir, ["d/a.nu"]);
    let output = format_in(&dir, &["d"]);
    assert_eq!(output.status.code(), Some(2));
    assert_eq!(
        stderr(&output),
        format!(
            "{}:1:11: `include` takes a list of strings, not an integer\n",
            nuon.display()
        )
    );
    assert_eq!(read(&path), UNFORMATTED);
}
