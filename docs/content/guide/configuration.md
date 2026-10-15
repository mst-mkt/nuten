---
title: Configuration
---

# Configuration

## Config file

Use one of these names. When several sit in the same directory, the order of precedence is `.nuon`, `.jsonc`, `.json`.

- `nuten.nuon`
- `nuten.jsonc`
- `nuten.json`

```nu [nuten.nuon]
{
    line_width: 100
    indent_style: space
    indent_width: 4
    commas: inline
}
```

See [Settings](/reference/settings) for every setting.

## Config file lookup

- The config file is looked up from the directory of the file being formatted, through each parent in turn, and the first one found is used
- The lookup stops at the directory holding `.git` or the like
- Without a config file, every setting takes its default

```nushell
nuten format --config ./ci/nuten.nuon
```

With `--config`, there is no lookup and that file is used.

From [treefmt-nix](/guide/integrations/treefmt#treefmt-nix), the settings can also go in the module's `settings`. \
From [dprint](/guide/integrations/dprint), they go in `dprint.json` instead of the config file. \
From [Prettier](/guide/integrations/prettier), they go in the Prettier config.

## Which files

```nu [nuten.nuon]
{
    include: ["*.nu", "*.nuon"]
    exclude: ["vendor", "scripts/generated/*.nu"]
    respect_gitignore: true
}
```

- `include`: the files picked when a directory is given
- `exclude`: files and directories left out, even when a file is given directly
- `respect_gitignore`: whether files ignored by `.gitignore` and the like are left out

## Overrides

```nu [nuten.nuon]
{
    line_width: 100
    overrides: [
        { include: ["tests/**/*.nu"], line_width: 120 }
    ]
}
```

Each entry picks files with `include` and `exclude`, and applies the rest of its settings to them. When a file is picked by several entries, the later one takes precedence.

## JSON Schema

`nuten schema` prints the JSON Schema of the config file.

```nushell
nuten schema | save nuten.schema.json
```

In JSON or JSONC, point `$schema` at it and your editor completes the settings.

```jsonc [nuten.jsonc]
{
  "$schema": "./nuten.schema.json",
  "line_width": 100,
}
```
