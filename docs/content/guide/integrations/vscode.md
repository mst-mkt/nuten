---
title: VS Code
---

# VS Code

## Installation

> [!NOTE]
> The extension is not yet published on the Marketplace or Open VSX.

The extension does not bundle `nuten`, and looks for it on the PATH. [Install](/guide/installation) `nuten` first.

The extension is published on the [Marketplace](https://marketplace.visualstudio.com/items?itemName=mst-mkt.nuten-vscode) and [Open VSX](https://open-vsx.org/extension/mst-mkt/nuten-vscode), so you can search for "Nuten" in the Extensions view, or install it with the following command.

::: code-group

```nushell [VS Code]
code --install-extension mst-mkt.nuten-vscode
```

```nushell [VSCodium]
codium --install-extension mst-mkt.nuten-vscode
```

```nushell [Cursor]
cursor --install-extension mst-mkt.nuten-vscode
```

:::

### Nix

You can add the extension from the flake of [mst-mkt/nuten](https://github.com/mst-mkt/nuten) or [nix-community/nix-vscode-extensions](https://github.com/nix-community/nix-vscode-extensions).

::: code-group

```nix [mst-mkt/nuten]
{
  nixpkgs.overlays = [ nuten.overlays.default ];
  programs.vscode.profiles.default.extensions = [
    pkgs.vscode-extensions.mst-mkt.nuten-vscode
  ];
}
```

```nix [nix-community/nix-vscode-extensions]
{
  nixpkgs.overlays = [ nix-vscode-extensions.overlays.default ];
  programs.vscode.profiles.default.extensions = [
    pkgs.vscode-marketplace.mst-mkt.nuten-vscode
  ];
}
```

:::

## Configuration

To format on save, set Nuten as the default formatter in `settings.json`.

```json [settings.json]
{
  "[nushell]": {
    "editor.defaultFormatter": "mst-mkt.nuten-vscode",
    "editor.formatOnSave": true
  },
  "[nuon]": {
    "editor.defaultFormatter": "mst-mkt.nuten-vscode",
    "editor.formatOnSave": true
  }
}
```

The extension has two settings.

| Setting              | Description                                                                                                          | Default           |
| -------------------- | -------------------------------------------------------------------------------------------------------------------- | ----------------- |
| `nuten.path`         | Path to the `nuten` executable                                                                                       | Looked up on PATH |
| `nuten.trace.server` | Logs the communication with the language server to `Nuten` in the Output panel<br>`"off"`, `"messages"`, `"verbose"` | `"off"`           |

Formatting settings go in the Nuten [config file](/guide/configuration), not in VS Code.
