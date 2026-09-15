//! Each `test_data/<name>.nu` must format to `test_data/<name>.expected.nu`,
//! which must format to itself, and formatting either must keep every
//! invariant in `nuten_format::invariants`.

use std::{fs, path::Path};

use nuten_format::{Config, format, invariants};

#[test]
fn fixtures_format_as_expected() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("test_data");
    let mut paths: Vec<_> = fs::read_dir(dir)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .collect();
    paths.sort();
    let mut checked = 0;
    for path in paths {
        let name = path.file_name().unwrap().to_str().unwrap();
        let Some(stem) = name.strip_suffix(".nu") else {
            continue;
        };
        if stem.ends_with(".expected") {
            continue;
        }
        let src = fs::read_to_string(&path).unwrap();
        let expected = fs::read_to_string(path.with_extension("expected.nu"))
            .unwrap_or_else(|error| panic!("{stem}.expected.nu: {error}"));
        check(name, &src, &expected);
        check(&format!("{stem}.expected.nu"), &expected, &expected);
        checked += 1;
    }
    assert!(checked > 0, "no fixtures");
}

fn check(name: &str, src: &str, expected: &str) {
    let config = Config::default();
    let actual = format(src, &config).unwrap_or_else(|error| panic!("{name}: {:?}", error.errors));
    assert!(
        actual == expected,
        "{name}\n--- expected\n{expected}--- actual\n{actual}--- as strings\n{expected:?}\n\
         {actual:?}"
    );
    let violations = invariants::violations(src, &config).expect("the source formatted above");
    assert!(
        violations.is_empty(),
        "{name}\n{}",
        violations
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join("\n")
    );
}
