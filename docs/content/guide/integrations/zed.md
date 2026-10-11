---
title: Zed
---

# Zed

## Installation

> [!NOTE]
> The extension is not yet published on the Zed extension registry.

The extension does not bundle `nuten`, and looks for it on the PATH. [Install](/guide/installation) `nuten` first.

In Zed, the Nushell language comes from the [Nu extension](https://github.com/zed-extensions/nu), so it needs to be installed too.

The extension is published on the Zed extension registry, so you can search for "Nuten" on the Extensions page and install it.

### Nix

With home-manager, add the extensions by their IDs.

```nix
{
  programs.zed-editor.extensions = [ "nu" "nuten-lsp" ];
}
```

## Configuration

Set Nuten as the formatter for Nu in `settings.json`.

```json [settings.json]
{
  "languages": {
    "Nu": {
      "formatter": { "language_server": { "name": "nuten" } }
    }
  }
}
```

To use a `nuten` that is not on the PATH, give its path.

```json [settings.json]
{
  "lsp": {
    "nuten": {
      "binary": { "path": "/path/to/nuten", "arguments": ["lsp"] }
    }
  }
}
```

Formatting settings go in the Nuten [config file](/guide/configuration), not in Zed.
