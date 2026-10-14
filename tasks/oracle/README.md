# nuten-oracle

The oracle test of [Nuten](https://github.com/mst-mkt/nuten), a formatter for Nushell.
It asks nu itself whether formatting keeps what nu parses, so that Nuten is checked against nu rather than against its own reading.

## Usage

The oracle runs the `nu` on the PATH, and needs it to be of the series that `VERSION` in `src/nu.rs` names.
`cargo oracle` runs it, as an alias in `.cargo/config.toml`.

```nushell
cargo oracle format path/to/dir
cargo oracle compare path/to/before.nu path/to/after.nu
```

`format` formats each `.nu` and `.nuon` file with the default settings, or with the config that `--config` names.
It also formats each file under every variant of `nuten_format::invariants::VARIANTS`, and checks the invariants of every output.
Then it has nu parse each file before and after formatting, and reports where the two ASTs differ.
For a `.nuon` file, it compares the values nu reads instead, and also checks that nuten-config reads the same value as nu.

`compare` asks nu whether two files parse the same, such as a file and another formatter's output for it.

## Dependencies

| Crate                                                 | Used for                                                   |
| ----------------------------------------------------- | ---------------------------------------------------------- |
| [nuten-format](../../crates/nuten_format)             | Formatting, and checking the invariants under each variant |
| [nuten-config](../../crates/nuten_config)             | Reading `--config`, and reading NUON to compare with nu    |
| [jsonc-parser](https://crates.io/crates/jsonc-parser) | Reading the ASTs and values nu prints as JSON              |
| [usage-rs](https://crates.io/crates/usage-rs)         | Parsing arguments                                          |

## Tests

`test_data` holds `.nuon` files with values that are easy to misread, and `test_data/nufmt` holds cases taken from nufmt's issues.

`cargo nextest` runs the unit tests, the CLI tests, and the nufmt cases.
Without nu on the PATH, the tests that need nu fail unless `NUTEN_TEST_SKIP_NU` is set.

`vp run test:oracle` runs the oracle on `test_data` and the corpus.
The `nix/dev` devShell sets `NUTEN_CORPUS` to the corpus.
Without Nix, assemble it with `nu scripts/prepare-corpus.nu path/to/corpus`, and set `NUTEN_CORPUS` to `path/to/corpus`.

```nushell
cargo nextest run --package nuten-oracle
vp run test:oracle
```
