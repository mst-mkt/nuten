# @nuten/prettier-plugin

The Prettier plugin of [Nuten](https://nuten.keito.dev), a formatter for Nushell.
It formats `.nu` and `.nuon` files, and Nushell code blocks in Markdown, through [@nuten/format](https://github.com/mst-mkt/nuten/tree/main/npm/format) without the `nuten` binary.

## Installation

```nushell
pnpm add --save-dev @nuten/prettier-plugin
```

```nushell
deno add --dev jsr:@nuten/prettier-plugin
```

## Usage

Add the plugin to the Prettier config.

```json [.prettierrc]
{
  "plugins": ["@nuten/prettier-plugin"]
}
```

Prettier does not read the [config file](https://nuten.keito.dev/guide/configuration) of Nuten.

## Options

The options of Prettier map to the [settings](https://nuten.keito.dev/reference/settings) of Nuten as follows.

| Option        | Setting        | Type                                           | Default    |
| ------------- | -------------- | ---------------------------------------------- | ---------- |
| `printWidth`  | `line_width`   | `number`                                       | `100`      |
| `useTabs`     | `indent_style` | `boolean`                                      | `false`    |
| `tabWidth`    | `indent_width` | `number`                                       | `4`        |
| `endOfLine`   | `line_ending`  | `"lf" \| "crlf" \| "auto"`                     | `"lf"`     |
| `nutenCommas` | `commas`       | `"none" \| "inline" \| "always" \| "trailing"` | `"inline"` |

- `useTabs: true` is `indent_style: "tab"`, and `useTabs: false` is `indent_style: "space"`.
- `printWidth` and `tabWidth` default to those of Nuten for `.nu` and `.nuon` files, unless the Prettier config or `.editorconfig` sets them. Code blocks in Markdown take the defaults of Prettier, `80` and `2`.
- `endOfLine: "cr"` is rejected, since Nushell does not end lines with CR alone.
- Line endings inside strings follow `endOfLine`, since Prettier changes every line ending before formatting. Nuten keeps them as written.
- `requirePragma` and `insertPragma` take `# @format` or `# @prettier` in the comments at the top of a file, and `checkIgnorePragma` takes `# @noformat` or `# @noprettier` there.

To set the options for Nushell files only, use `overrides`.

```json [.prettierrc]
{
  "plugins": ["@nuten/prettier-plugin"],
  "overrides": [
    {
      "files": ["*.nu", "*.nuon"],
      "options": { "printWidth": 120 }
    }
  ]
}
```

## License

[MIT](https://github.com/mst-mkt/nuten/blob/main/LICENSE)
