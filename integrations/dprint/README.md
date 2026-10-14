# nuten-dprint

The dprint plugin of [Nuten](https://github.com/mst-mkt/nuten), a formatter for Nushell.
It builds [nuten-format](../../crates/nuten_format) into a Wasm plugin, so that dprint formats `.nu` and `.nuon` files without the `nuten` binary.

## Usage

```nushell
dprint config add mst-mkt/nuten
```

The settings go under `"nuten"` in `dprint.json`.
Setup is covered in the [dprint guide](https://nuten.keito.dev/guide/integrations/dprint).

## Dependencies

| Crate                                               | Used for                                                           |
| --------------------------------------------------- | ------------------------------------------------------------------ |
| [nuten-format](../../crates/nuten_format)           | Formatting                                                         |
| [nuten-line-index](../../crates/nuten_line_index)   | Pointing at syntax errors by line and column                       |
| [dprint-core](https://crates.io/crates/dprint-core) | The interface of a dprint Wasm plugin                              |
| [serde](https://crates.io/crates/serde)             | Serializing the resolved config for dprint                         |
| [serde_json](https://crates.io/crates/serde_json)   | The plugin glue code, and checking `schema.json` in the unit tests |

## Development

The `nix/dev` devShell provides the `wasm32-unknown-unknown` target.

```nushell
cargo build --release --package nuten-dprint --target wasm32-unknown-unknown
```

## Tests

The unit tests in `src/lib.rs` check how the config resolves, and that it agrees with `schema.json`, the JSON Schema of the settings.
`tests/dprint.nu` is a [nutest](https://github.com/vyadh/nutest) suite that runs dprint with the built plugin on temporary directories.
`test.nu` runs the suite with the plugin that `NUTEN_DPRINT` names, and builds one first when it names none.

```nushell
cargo nextest run --package nuten-dprint
vp run test:dprint
```
