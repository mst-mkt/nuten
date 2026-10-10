---
title: Getting started
---

# Getting started

## Formatting files

```nushell
nuten format
```

Formats every `.nu` and `.nuon` file under the current directory. Pass files or directories to format only those.

```nushell
nuten format src scripts/build.nu
```

## Checking without formatting

```nushell
nuten format --check
```

Files that would change are shown like this.

```ansi
[1m2 files would be reformatted:[0m

[2m╭─ [0m[1mmain.nu:2[0m
[2m│ [0m  [2mdef greet [name] {[0m
[2m│ [0m[31m▍ [0m[2m[31m··[0m[31m$"hello[0m[2m[31m·[0m[31m($name)"[0m
[2m│ [0m[32m▍ [0m[2m[32m····[0m[32m$"hello[0m[2m[32m·[0m[32m($name)"[0m
[2m│ [0m  [2m}[0m
[2m│ [0m
[2m│ [0m[31m▍ [0m[31mlet[0m[2m[31m·[0m[31mx[0m[2m[31m·[0m[31m=[0m[2m[31m·[0m[31m{a:1,b:[1[0m[2m[31m·[0m[31m2[0m[2m[31m·[0m[31m3]}[0m
[2m│ [0m[32m▍ [0m[32mlet[0m[2m[32m·[0m[32mx[0m[2m[32m·[0m[32m=[0m[2m[32m·[0m[32m{[0m[2m[32m·[0m[32ma:[0m[2m[32m·[0m[32m1,[0m[2m[32m·[0m[32mb:[0m[2m[32m·[0m[32m[1[0m[1m[32m,[0m[2m[32m·[0m[32m2[0m[1m[32m,[0m[2m[32m·[0m[32m3][0m[2m[32m·[0m[32m}[0m
[2m╰─[0m
[2m╭─ [0m[1mok.nu:1[0m
[2m│ [0m[31m▍ [0m[31mdef[0m[2m[31m·[0m[31mok[0m[2m[31m·[0m[31m[][0m[2m[31m·[0m[31m{[0m[2m[31m·[0m[31m1[0m[2m[31m·[0m[31m}[0m
[2m│ [0m[32m▍ [0m[32mdef[0m[2m[32m·[0m[32mok[0m[2m[32m·[0m[32m[][0m[2m[32m·[0m[32m{[0m[2m[32m⏎[0m
[2m│ [0m[32m▍ [0m[2m[32m····[0m[32m1[0m[2m[32m⏎[0m
[2m│ [0m[32m▍ [0m[32m}[0m
[2m╰─[0m
[1m2[0m files: [1m2[0m need formatting (5 lines, 4 whitespace only)
```

## Next steps

- [Configuration](/guide/usage/configuration)
- [Integrations](/guide/integrations)
- [CLI reference](/reference/cli)
