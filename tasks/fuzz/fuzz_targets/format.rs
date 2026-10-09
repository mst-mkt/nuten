#![no_main]

use std::iter;

use libfuzzer_sys::fuzz_target;
use nuten_format::{
    Config,
    invariants::{self, VARIANTS},
};

fuzz_target!(|src: &str| {
    let config = Config::default();
    let configs = iter::once(("default", config.clone())).chain(
        VARIANTS
            .iter()
            .map(|variant| (variant.name, variant.of(&config))),
    );
    let outputs: Vec<_> = configs
        .map(|(name, config)| {
            let checked = invariants::check(src, &config);
            assert!(
                checked.violations.is_empty(),
                "{name}: {src:?}: {:?}",
                checked.violations
            );
            (name, checked.output.is_some())
        })
        .collect();
    assert!(
        outputs.iter().all(|&(_, given)| given == outputs[0].1),
        "output given under some configs only: {src:?}: {outputs:?}"
    );
});
