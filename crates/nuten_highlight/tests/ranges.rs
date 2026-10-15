use std::{
    fs,
    path::{Path, PathBuf},
};

use nuten_highlight::highlight;
use nuten_syntax::{NodeOrToken, TextRange};

#[test]
fn ranges_come_in_order_each_within_one_token() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let mut paths: Vec<_> = ["nuten_parser/test_data", "nuten_format/test_data"]
        .into_iter()
        .flat_map(|dir| nu_files_below(&root.join(dir)))
        .collect();
    paths.sort();
    assert!(!paths.is_empty(), "no fixtures below {}", root.display());

    for path in paths {
        let src =
            fs::read_to_string(&path).unwrap_or_else(|error| panic!("{}: {error}", path.display()));
        let tree = nuten_parser::parse(&src).syntax();
        let tokens: Vec<_> = tree
            .descendants_with_tokens()
            .filter_map(NodeOrToken::into_token)
            .map(|token| token.text_range())
            .collect();
        let mut end = 0.into();
        for (range, token_type) in highlight(&tree) {
            let at = format!("{} at {range:?} ({token_type:?})", path.display());
            assert!(!range.is_empty(), "empty range in {at}");
            assert!(range.start() >= end, "range out of order in {at}");
            assert!(
                tokens
                    .iter()
                    .any(|token: &TextRange| token.contains_range(range)),
                "range across tokens in {at}"
            );
            end = range.end();
        }
    }
}

fn nu_files_below(dir: &Path) -> Vec<PathBuf> {
    let entries = fs::read_dir(dir).unwrap_or_else(|error| panic!("{}: {error}", dir.display()));
    entries
        .map(|entry| {
            entry
                .unwrap_or_else(|error| panic!("{}: {error}", dir.display()))
                .path()
        })
        .flat_map(|path| {
            if path.is_dir() {
                nu_files_below(&path)
            } else if path.extension().is_some_and(|extension| extension == "nu") {
                vec![path]
            } else {
                Vec::new()
            }
        })
        .collect()
}
