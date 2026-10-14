# nuten-format

The formatting core of [Nuten](https://github.com/mst-mkt/nuten), a formatter for Nushell.
It parses a source with [nuten-parser](../nuten_parser), turns the tree into a document of [nuten-doc](../nuten_doc), and prints it.

## Usage

```rust
use nuten_format::{Config, format};

let output = format("def greet [name] {$\"hello ($name)\"}", &Config::default()).unwrap();
assert_eq!(output, "def greet [name] {\n    $\"hello ($name)\"\n}\n");
```

`format` returns an error and no output when the source has syntax errors.
`invariants::check` formats a source and also reports where the output loses a token or a comment, or changes when formatted again.

## Structure

The rules live in `src/rules`, mostly one file for each kind of syntax.

## Dependencies

| Crate                           | Used for                                                    |
| ------------------------------- | ----------------------------------------------------------- |
| [nuten-parser](../nuten_parser) | Parsing the source, and checking the invariants of the tree |
| [nuten-syntax](../nuten_syntax) | Reading the tree in the rules                               |
| [nuten-doc](../nuten_doc)       | Building the document and printing it                       |

## Development

The dprint plugin builds this crate for `wasm32-unknown-unknown`, so `vp run lint:rust` also runs clippy on it for that target.

## Tests

`test_data` holds fixtures, each a pair of `{name}.input.nu` and `{name}.expected.nu`.
A fixture passes when the input formats to the expected output, the expected output formats to itself, and both keep the invariants.
Fixtures under `test_data/config/{setting}/{value}` are formatted with that setting changed from the default.
A new directory there needs an entry in `CONFIGS` of `tests/fixtures.rs`.

`tests/corpus.rs` formats every file in the corpus, and checks the invariants.
Without `NUTEN_CORPUS`, the test fails unless `NUTEN_TEST_SKIP_CORPUS` is set.

The `nix/dev` devShell sets `NUTEN_CORPUS` to the corpus.
Without Nix, assemble it with `nu scripts/prepare-corpus.nu path/to/corpus`, and set `NUTEN_CORPUS` to `path/to/corpus`.

```nushell
cargo nextest run --package nuten-format
```
