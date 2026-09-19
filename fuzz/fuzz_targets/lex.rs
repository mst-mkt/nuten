//! In every mode, the lexer must not panic, and must keep every lexer invariant in `nuten_parser::invariants`.

#![no_main]

use libfuzzer_sys::fuzz_target;
use nuten_parser::invariants;

fuzz_target!(|src: &str| {
    let violations = invariants::lexer_violations(src);
    assert!(violations.is_empty(), "{src:?}: {violations:?}");
});
