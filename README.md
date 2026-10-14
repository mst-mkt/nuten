<div align="center">

<img src="./assets/logo.svg" alt="Nuten logo" width="150" />

# Nuten

A formatter for Nushell. Robust, fast, and lightweight.

[Documentation](https://nuten.keito.dev) | [Playground](https://nuten.keito.dev/playground)

</div>

Nuten is not an official Nushell project.
The official formatter is [nufmt](https://github.com/nushell/nufmt).
The documentation [introduces Nuten](https://nuten.keito.dev/about/what-is-nuten), including how it differs from nufmt.

## Installation

```nushell
cargo install --git https://github.com/mst-mkt/nuten nuten
```

```nushell
nix profile install github:mst-mkt/nuten
```

Other methods are covered in the [installation guide](https://nuten.keito.dev/guide/installation).

## Usage

```nushell
nuten format                      # format .nu and .nuon under the current directory
nuten format src a.nu             # format the given directories and files
nuten format --check              # show what would change without writing
open --raw a.nu | nuten format -  # format stdin and write to stdout
```

Options and output are covered in the [formatting guide](https://nuten.keito.dev/guide/formatting).

## Integrations

> [!NOTE]
> Some integrations are not yet published.

- Editors
  - [VS Code](https://nuten.keito.dev/guide/integrations/vscode) extension on the [Marketplace](https://marketplace.visualstudio.com/items?itemName=mst-mkt.nuten-vscode) and [Open VSX](https://open-vsx.org/extension/mst-mkt/nuten-vscode)
  - [Zed](https://nuten.keito.dev/guide/integrations/zed) extension
  - [Vim](https://nuten.keito.dev/guide/integrations/vim), [Neovim](https://nuten.keito.dev/guide/integrations/neovim), [Helix](https://nuten.keito.dev/guide/integrations/helix), and [Emacs](https://nuten.keito.dev/guide/integrations/emacs) through the [language server](https://nuten.keito.dev/reference/cli#nuten-lsp)
- Tools
  - [dprint](https://nuten.keito.dev/guide/integrations/dprint) Wasm plugin
  - [treefmt](https://nuten.keito.dev/guide/integrations/treefmt) module for treefmt-nix
  - [GitHub Actions](https://nuten.keito.dev/guide/integrations/ci#github-actions) and [GitLab CI](https://nuten.keito.dev/guide/integrations/ci#gitlab-ci) reporters

## Contributing

See [CONTRIBUTING.md](./CONTRIBUTING.md) for details.

## License

[MIT](./LICENSE)
