//! The parser must not panic; its tree must restore the input, with no node
//! that is empty or starts or ends with trivia; and its errors must match
//! its `Error` elements one to one, each inside its element.

#![no_main]

use libfuzzer_sys::fuzz_target;
use nuten_parser::parse;
use nuten_syntax::SyntaxKind;

fuzz_target!(|src: &str| {
    let parsed = parse(src);
    let root = parsed.syntax();
    assert_eq!(root.text(), src, "the tree does not restore {src:?}");

    for node in root.descendants().skip(1) {
        for token in [node.first_token(), node.last_token()] {
            assert!(
                token.is_some_and(|token| !token.kind().is_trivia()),
                "{:?} is empty or starts or ends with trivia in {src:?}",
                node.kind()
            );
        }
    }

    let error_ranges: Vec<_> = root
        .descendants_with_tokens()
        .filter(|element| element.kind() == SyntaxKind::Error)
        .map(|element| element.text_range())
        .collect();
    assert_eq!(
        error_ranges.len(),
        parsed.errors().len(),
        "errors do not match Error elements in {src:?}"
    );
    for (range, error) in error_ranges.into_iter().zip(parsed.errors()) {
        assert!(
            range.contains_range(error.range),
            "misplaced {error:?} in {src:?}"
        );
    }
});
