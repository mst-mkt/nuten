#![no_main]

use libfuzzer_sys::fuzz_target;
use nuten_parser::{invariants, parse};

fuzz_target!(|src: &str| {
    let violations = invariants::violations(src, &parse(src));
    assert!(violations.is_empty(), "{src:?}: {violations:?}");
});
