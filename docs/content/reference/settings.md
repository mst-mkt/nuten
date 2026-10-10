---
title: Settings
---

# Settings

## line_width

**Type** `integer` \
**Default** `100`

Specifies the maximum number of columns per line. Longer lines are broken where they can be.

## indent_style

**Type** `"space" | "tab"` \
**Default** `"space"`

Specifies the character to use for indentation.

## indent_width

**Type** `integer` \
**Default** `4` \
**Range** 1 ~ 255

Specifies the number of columns per indentation level.

When indenting with tabs, each tab counts as this many columns toward the line width.

## line_ending

**Type** `"lf" | "crlf" | "auto"` \
**Default** `"auto"`

Specifies the line ending. With `"auto"`, it follows the first line ending in the file.

Line endings inside strings and attribute blocks are kept as written, regardless of this setting.

## commas

**Type** `"none" | "inline" | "always" | "trailing"` \
**Default** `"inline"`

Specifies where commas go between the items of lists, records, and the like.

::: compare

```nushell [none]
[1 2 3]

[
    "src/main.nu"
    "tests/main.nu"
]
```

```nushell [inline]
[1, 2, 3]

[
    "src/main.nu"
    "tests/main.nu"
]
```

```nushell [always]
[1, 2, 3]

[
    "src/main.nu",
    "tests/main.nu"
]
```

```nushell [trailing]
[1, 2, 3]

[
    "src/main.nu",
    "tests/main.nu",
]
```

:::

## include

**Type** `string[]` \
**Default** `["*.nu", "*.nuon"]`

Specifies the files to format, as glob patterns. A file given directly is formatted even if it does not match.

Patterns are relative to the directory of the config file. A pattern without `/` matches file names at any depth.

## exclude

**Type** `string[]` \
**Default** `[]`

Specifies the files and directories to leave out, as glob patterns. This applies to files given directly too.

Patterns are relative to the directory of the config file. A pattern without `/` matches file and directory names at any depth.

## respect_gitignore

**Type** `boolean` \
**Default** `true`

Specifies whether to skip files listed in `.gitignore` and the like.

## overrides

**Type** `{ include: string[], exclude?: string[], line_width?, indent_style?, indent_width?, line_ending?, commas? }[]` \
**Default** `[]`

Changes the settings for some files only. Each element has the following fields.

| Field                                                                 | Type                  | Description                                        |
| --------------------------------------------------------------------- | --------------------- | -------------------------------------------------- |
| `include`                                                             | `string[]`            | Patterns of the files to apply to                  |
| `exclude`                                                             | `string[]`            | Patterns of the files and directories to leave out |
| `line_width`, `indent_style`, `indent_width`, `line_ending`, `commas` | Same as the top level | Settings for the matched files                     |

```nu [nuten.nuon]
{
    overrides: [
        { include: ["tests/**/*.nu"], line_width: 120 }
    ]
}
```

When several elements match a file, they apply in order, and the later one wins for a setting they both set.
