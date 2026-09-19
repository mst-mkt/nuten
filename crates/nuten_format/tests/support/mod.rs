//! Shared by the tests that run nuten over many files.

use std::panic::{self, UnwindSafe};

/// What `check` gives, or the message of the panic it raised.
/// A panic is a bug in nuten itself; catching it keeps the rest of the files
/// checked.
pub(crate) fn caught<T>(check: impl FnOnce() -> T + UnwindSafe) -> Result<T, String> {
    panic::catch_unwind(check).map_err(|payload| {
        payload
            .downcast_ref::<String>()
            .map(String::as_str)
            .or_else(|| payload.downcast_ref::<&str>().copied())
            .unwrap_or("a panic")
            .to_owned()
    })
}
