# nuten

The command-line interface of [Nuten](https://github.com/mst-mkt/nuten), a formatter for Nushell.
It builds the `nuten` binary, which finds files, formats them, and reports the result.

## Usage

| Subcommand          | What it does                                                |
| ------------------- | ----------------------------------------------------------- |
| `nuten format`      | Formats files, or reports what would change without writing |
| `nuten schema`      | Prints the JSON Schema of the config file                   |
| `nuten lsp`         | Runs the language server over stdio                         |
| `nuten completions` | Prints shell completions                                    |

The options of each subcommand are listed in the [CLI reference](https://nuten.keito.dev/reference/cli).

## Dependencies

| Crate                                                                                      | Used for                                                                         |
| ------------------------------------------------------------------------------------------ | -------------------------------------------------------------------------------- |
| [nuten-config](../nuten_config)                                                            | Finding and reading the config file of each file, and generating the JSON Schema |
| [nuten-format](../nuten_format)                                                            | Formatting                                                                       |
| [nuten-line-index](../nuten_line_index)                                                    | Pointing at syntax errors by line and column                                     |
| [nuten-lsp](../nuten_lsp)                                                                  | Running the language server for `nuten lsp`                                      |
| [ignore](https://crates.io/crates/ignore)                                                  | Walking directories while following `.gitignore`                                 |
| [usage-rs](https://crates.io/crates/usage-rs)                                              | Parsing arguments, and generating help and shell completions                     |
| [serde](https://crates.io/crates/serde), [serde_json](https://crates.io/crates/serde_json) | Writing the GitLab Code Quality report                                           |

## Development

```nushell
cargo run --package nuten -- format --check path/to/file.nu
cargo build --release --package nuten
```

## Tests

`tests/cli` runs the built binary on temporary directories, and checks the files it rewrites, what it prints, and its exit code.

```nushell
cargo nextest run --package nuten
```
