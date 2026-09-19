//! In every mode, the lexer must not panic, and must keep every lexer
//! invariant in `nuten_parser::invariants`.

#![no_main]

use libfuzzer_sys::fuzz_target;
use nuten_parser::{invariants, lexer::Mode};

fuzz_target!(|src: &str| {
    for &mode in Mode::ALL {
        let violations = invariants::lexer_violations(src, mode);
        assert!(violations.is_empty(), "{src:?} in {mode:?}: {violations:?}");
    }
});
