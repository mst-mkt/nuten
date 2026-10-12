---
title: dprint
---

# dprint

## Installation

> [!NOTE]
> The plugin is not yet included in the GitHub releases, and is not yet listed on [dprint's plugin list](https://dprint.dev/plugins/).

Nuten works as a dprint Wasm plugin. The plugin contains Nuten's formatter, so you do not need to install `nuten`.

Add the plugin to `dprint.json` with the following command.

::: code-group

```nushell [Latest]
dprint config add mst-mkt/nuten
```

```nushell [Specific version]
dprint config add https://plugins.dprint.dev/mst-mkt/nuten-{version}.wasm
```

:::

## Configuration

Write the settings under `"nuten"` in `dprint.json`. When you use Nuten from dprint, it does not read the project's [config file](/guide/configuration).

```json [dprint.json]
{
  "nuten": {
    "lineWidth": 80,
    "indentWidth": 2
  }
}
```

The setting names follow other dprint plugins and map to the config file as follows. See [Settings](/reference/settings) for what each setting does.

| dprint        | Config file    | Values                                    |
| ------------- | -------------- | ----------------------------------------- |
| `lineWidth`   | `line_width`   | Same                                      |
| `useTabs`     | `indent_style` | `true` for `"tab"`, `false` for `"space"` |
| `indentWidth` | `indent_width` | Same                                      |
| `newLineKind` | `line_ending`  | Same                                      |
| `commas`      | `commas`       | Same                                      |

If `lineWidth`, `useTabs`, `indentWidth`, or `newLineKind` is not set in `"nuten"`, the global dprint setting at the top level of `dprint.json` is used.

The plugin formats `.nu` and `.nuon` files. To leave files out, use dprint's `excludes`.
