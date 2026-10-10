---
title: Formatting
---

# Formatting

## Files to format

```nushell
nuten format            # .nu and .nuon under the current directory
nuten format src        # .nu and .nuon under src
nuten format a.nu b.nu  # the given files
```

Given a directory, these are left out.

- Files ignored by `.gitignore` and the like
- Files under VCS directories such as `.git`
- Symbolic links, which are not followed

A file given directly is formatted whatever its extension. `include`, `exclude`, and `respect_gitignore` in the [configuration](/guide/usage/configuration) change which files are picked.

## Writing files

Files are rewritten in place, so hard links and permissions stay as they are. If a write fails, the original contents are put back.

At the end, a one-line summary goes to stderr.

```
10 files: 2 formatted, 1 failed, 3 skipped, 4 excluded
```

- `formatted`: rewritten
- `unchanged`: nothing to change
- `failed`: a syntax error or a read or write error
- `skipped`: left alone because a config file is wrong
- `excluded`: left out by `exclude` or `.gitignore`

## Checking without formatting

```nushell
nuten format --check
```

Shows what would change on stdout and leaves the files alone.

```ansi
[2m╭─ [0m[1mmain.nu:2[0m
[2m│ [0m  [2mdef greet [name] {[0m
[2m│ [0m[31m▍ [0m[2m[31m··[0m[31m$"hello[0m[2m[31m·[0m[31m($name)"[0m
[2m│ [0m[32m▍ [0m[2m[32m····[0m[32m$"hello[0m[2m[32m·[0m[32m($name)"[0m
[2m│ [0m  [2m}[0m
[2m╰─[0m
```

- Red is the line before, green the line after. Without color, they are marked `-` and `+`
- `·` is a space, `→` a tab, and `⏎` a line break added or removed
- A file whose only change is its line endings shows `line endings: CRLF → LF`

```nushell
nuten format --list-different
```

Prints only the paths of the files that would change, one per line, for scripts to read.

Either way, the exit code is 1 when some file would change. To report the result in CI, use [`--reporter`](/guide/integrations/ci).

## stdin

```nushell
open --raw a.nu | nuten format -
```

`-` formats stdin and writes the result to stdout. The config file is looked up from the current directory, and `overrides` and `exclude` do not apply.

When the content belongs to a known file, pass its path with `--stdin-filepath`. The config file is then looked up from that path, and `overrides` and `exclude` apply too.

```nushell
$buffer | nuten format --stdin-filepath src/a.nu -
```

## Exit codes

| code | Condition                                                            |
| ---- | -------------------------------------------------------------------- |
| 0    | Success                                                              |
| 1    | Some file would change (`--check`, `--list-different`, `--reporter`) |
| 2    | A syntax error, a read or write error, or a wrong config or argument |

A file with a syntax error is not formatted. The error goes to stderr as `error: path:line:column: message`, and the other files are still processed.
