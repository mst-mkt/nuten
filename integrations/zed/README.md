# nuten-zed

The Zed extension of [Nuten](https://github.com/mst-mkt/nuten), a formatter for Nushell.
It runs `nuten lsp` from the PATH as a language server for Nushell.
In Zed, the Nushell language comes from the [Nu extension](https://github.com/zed-extensions/nu), which needs to be installed too.

## Usage

The extension does not bundle `nuten`, so [install](https://nuten.keito.dev/guide/installation) it first.
Setup is covered in the [Zed guide](https://nuten.keito.dev/guide/integrations/zed).

## Development

The extension is a crate of its own outside the workspace, depends only on [zed_extension_api](https://crates.io/crates/zed_extension_api), and has its own `Cargo.lock`.
Zed builds it for `wasm32-wasip2`, which the `nix/dev` devShell provides.

```nushell
cargo build --manifest-path integrations/zed/Cargo.toml --target wasm32-wasip2
```

To try a local build, run `zed: install dev extension` in Zed and pick this directory.

## Tests

```nushell
vp run test:zed
```
