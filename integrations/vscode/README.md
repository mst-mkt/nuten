# nuten-vscode

The VS Code extension of [Nuten](https://nuten.keito.dev), a formatter for Nushell.
It formats `.nu` and `.nuon` files through `nuten lsp`.

## Installation

The extension does not bundle `nuten`, and looks for it on the PATH.
[Install](https://nuten.keito.dev/guide/installation) `nuten` first, then install the extension from the [Marketplace](https://marketplace.visualstudio.com/items?itemName=mst-mkt.nuten-vscode) or [Open VSX](https://open-vsx.org/extension/mst-mkt/nuten-vscode).

## Configuration

To format on save, set Nuten as the default formatter in `settings.json`.

```json
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

| Setting              | Description                                                                                                                         | Default   |
| -------------------- | ----------------------------------------------------------------------------------------------------------------------------------- | --------- |
| `nuten.path`         | Path to the `nuten` executable                                                                                                      | `"nuten"` |
| `nuten.trace.server` | How much of the communication with the language server to log to `Nuten` in the Output panel: `"off"`, `"messages"`, or `"verbose"` | `"off"`   |

In an untrusted workspace, the workspace value of `nuten.path` is ignored.

Formatting settings go in the Nuten [config file](https://nuten.keito.dev/guide/configuration), not in VS Code.
Other ways to install, such as with Nix, are covered in the [VS Code guide](https://nuten.keito.dev/guide/integrations/vscode).
