---
title: Prettier
---

# Prettier

## Installation

> [!NOTE]
> The plugin is not yet published on npm or JSR.

Nuten works as a Prettier plugin. The plugin does not call the `nuten` command, so you do not need to install `nuten`.

::: code-group

```nushell [Vite+]
vp i --dev @nuten/prettier-plugin
```

```nushell [npm]
npm install --save-dev @nuten/prettier-plugin
```

```nushell [yarn]
yarn add --dev @nuten/prettier-plugin
```

```nushell [pnpm]
pnpm add --save-dev @nuten/prettier-plugin
```

```nushell [Deno]
deno add --dev jsr:@nuten/prettier-plugin
```

```nushell [bun]
bun add --dev @nuten/prettier-plugin
```

:::

Then add the plugin to the Prettier config.

```json [.prettierrc]
{
  "plugins": ["@nuten/prettier-plugin"]
}
```

Prettier now formats `.nu` and `.nuon` files, scripts whose shebang runs `nu`, and Markdown code blocks marked `nu` or `nushell`.

## Configuration

Write the settings in the Prettier config. When you use Nuten from Prettier, it does not read the project's [config file](/guide/configuration).

Prettier's options map to the config file as follows. See [Settings](/reference/settings) for what each setting does.

| Prettier      | Config file    | Values                                    | Default    |
| ------------- | -------------- | ----------------------------------------- | ---------- |
| `printWidth`  | `line_width`   | Same                                      | `100`      |
| `useTabs`     | `indent_style` | `true` for `"tab"`, `false` for `"space"` | `false`    |
| `tabWidth`    | `indent_width` | Same                                      | `4`        |
| `endOfLine`   | `line_ending`  | Same, except that `"cr"` is rejected      | `"lf"`     |
| `nutenCommas` | `commas`       | Same                                      | `"inline"` |

In Markdown code blocks, `printWidth` and `tabWidth` take Prettier's defaults instead.
