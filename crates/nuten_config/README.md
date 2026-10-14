# nuten-config

The config file loader of [Nuten](https://github.com/mst-mkt/nuten), a formatter for Nushell.
It finds the config file that applies to each file, reads it into settings, and generates the JSON Schema of the config file.

## Usage

`load` reads the text of a config file into settings.

```rust
use nuten_config::{Format, load};

let config = load("{ line_width: 80 }", Format::Nuon).unwrap();
assert_eq!(config.format.line_width, 80);
```

`of_files`, `of_dirs`, and `of_text` find the nearest config file of files, directories, and stdin, and load it.
`of_files` and `of_dirs` use a given config instead when there is one, such as the one `--config` names.
What each setting means is described in the [settings reference](https://nuten.keito.dev/reference/settings).

## Dependencies

| Crate                                                                                      | Used for                                        |
| ------------------------------------------------------------------------------------------ | ----------------------------------------------- |
| [nuten-format](../nuten_format)                                                            | Holding the formatting settings                 |
| [nuten-parser](../nuten_parser), [nuten-syntax](../nuten_syntax)                           | Reading NUON config files as Nushell reads them |
| [nuten-line-index](../nuten_line_index)                                                    | Pointing at errors in a config file             |
| [jsonc-parser](https://crates.io/crates/jsonc-parser)                                      | Reading JSONC and JSON config files             |
| [globset](https://crates.io/crates/globset)                                                | Matching `include`, `exclude`, and `overrides`  |
| [serde](https://crates.io/crates/serde), [serde_json](https://crates.io/crates/serde_json) | Writing the JSON Schema                         |
| [insta](https://crates.io/crates/insta) (dev)                                              | The snapshot of the JSON Schema                 |

## Tests

`tests/find.rs` creates config files in temporary directories, and checks which one each directory picks.
`tests/schema.rs` checks that every value the JSON Schema allows loads, and every value it rejects fails to load.
It also keeps the JSON Schema as an [insta](https://insta.rs) snapshot, so that a change to it shows up for review.

```nushell
cargo nextest run --package nuten-config
cargo insta review
```
