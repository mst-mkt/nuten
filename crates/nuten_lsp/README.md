# nuten-lsp

The language server of [Nuten](https://github.com/mst-mkt/nuten), a formatter for Nushell.
It formats documents for editors over the Language Server Protocol, with the settings that [nuten-config](../nuten_config) finds and the rules of [nuten-format](../nuten_format).

## Usage

`nuten lsp` calls `serve_stdio`, which serves one client over stdin and stdout until the client exits or disconnects.
The server supports document formatting only.

How to register it in each editor is described in the [integration guides](https://nuten.keito.dev/guide/integrations).

## Dependencies

| Crate                                                                                              | Used for                                                 |
| -------------------------------------------------------------------------------------------------- | -------------------------------------------------------- |
| [nuten-config](../nuten_config)                                                                    | Finding the config file of each document                 |
| [nuten-format](../nuten_format)                                                                    | Formatting                                               |
| [lsp-server](https://crates.io/crates/lsp-server), [lsp-types](https://crates.io/crates/lsp-types) | Handling the connection and the messages of the protocol |
| [serde_json](https://crates.io/crates/serde_json)                                                  | Reading the parameters of the messages                   |

## Tests

`tests/serve` runs the server through `serve` on an in-memory connection, and checks the lifecycle and the edits it returns.

```nushell
cargo nextest run --package nuten-lsp
```
