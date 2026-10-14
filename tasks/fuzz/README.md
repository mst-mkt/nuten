# nuten-fuzz

The fuzz targets of [Nuten](https://github.com/mst-mkt/nuten), a formatter for Nushell.
They feed random input to [nuten-parser](../../crates/nuten_parser) and [nuten-format](../../crates/nuten_format) through [cargo-fuzz](https://github.com/rust-fuzz/cargo-fuzz).

## Usage

| Target   | What it checks                                                                                                                          |
| -------- | --------------------------------------------------------------------------------------------------------------------------------------- |
| `lex`    | The lexer does not panic, and its tokens keep the invariants of the lexer                                                               |
| `parse`  | The parser does not panic, and its tree keeps the invariants of the parser                                                              |
| `format` | The formatter does not panic, keeps the invariants under the defaults and each variant, and formats the input under all or none of them |

cargo-fuzz needs a nightly toolchain, which the `fuzz` devShell of `nix/dev` provides.

```nushell
nix develop ./nix/dev#fuzz --command cargo fuzz run --fuzz-dir tasks/fuzz format
```

`vp run test:fuzz` runs each target for a minute.
CI also fuzzes each target every day.
An input that fails lands in `artifacts/{target}`.

## Dependencies

| Crate                                                   | Used for                            |
| ------------------------------------------------------- | ----------------------------------- |
| [nuten-parser](../../crates/nuten_parser)               | The `lex` and `parse` targets       |
| [nuten-format](../../crates/nuten_format)               | The `format` target                 |
| [libfuzzer-sys](https://crates.io/crates/libfuzzer-sys) | Running the targets under libFuzzer |

## Development

The fuzz targets are a crate of their own outside the workspace, with their own `Cargo.lock`.
