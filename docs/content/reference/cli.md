---
title: CLI
---

# CLI

```nushell
nuten <SUBCOMMAND>
```

| Subcommand    | Description                               |
| ------------- | ----------------------------------------- |
| `format`      | Format files                              |
| `schema`      | Output the JSON Schema of the config file |
| `lsp`         | Start the language server                 |
| `completions` | Output a shell completion script          |

## `nuten format`

```nushell
nuten format [FLAGS] [PATHS]...
```

| Argument | Description                                                                                                                                                                             | Default |
| -------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------- |
| `PATHS`  | Specify files or directories to format<br>If a directory is specified, format the `.nu` and `.nuon` files under it<br>If `-` is specified, format stdin and output the result to stdout | `.`     |

| Flag                             | Description                                                                                                        |
| -------------------------------- | ------------------------------------------------------------------------------------------------------------------ |
| `--config <PATH>`<br>`-c <PATH>` | Specify the config file to use<br>If not specified, search upward from the directory of each file                  |
| `--check`                        | Output the diff to stdout without rewriting files                                                                  |
| `--list-different`<br>`-l`       | Output the paths of files with differences to stdout, one per line, without rewriting files                        |
| `--reporter <github\|gitlab>`    | Output diffs and errors to stdout in the format of the specified CI, without rewriting files                       |
| `--stdin-filepath <PATH>`        | Specify which file the stdin content belongs to<br>Used to find the config file and to name the file in the output |

Errors are output to stderr as `error: <message>`.

At the end, a summary of the processed files is output to stderr. It is not output when formatting stdin or with `--list-different`.

```
3 files: 2 need formatting (5 lines, 3 whitespace only), 1 unchanged
```

### Exit codes

| Code | Formatting | Checking (`--check`, `--list-different`, `--reporter`) |
| ---- | ---------- | ------------------------------------------------------ |
| 0    | Success    | No differences                                         |
| 1    |            | Differences found                                      |
| 2    | Error      | Error                                                  |

Errors include syntax errors, file read/write failures, config or argument errors, and internal errors during formatting.

### Environment variables

| Variable         | Description                                                                 |
| ---------------- | --------------------------------------------------------------------------- |
| `NO_COLOR`       | If non-empty, do not color output<br>Takes precedence over `CLICOLOR_FORCE` |
| `CLICOLOR_FORCE` | If not `0`, color output even when it is not a terminal                     |

### Examples

```nushell
nuten format                                            # Format files
nuten format --check                                    # Output diffs without rewriting
nuten format --reporter github                          # Output diffs and errors as GitHub Actions annotations
$text | nuten format --stdin-filepath src/a.nu -        # Format stdin as the content of src/a.nu
```

## `nuten schema`

```nushell
nuten schema
```

Outputs the JSON Schema of the config file to stdout.

```nushell
nuten schema | save nuten.schema.json
```

## `nuten lsp`

```nushell
nuten lsp
```

Starts the language server, communicating via stdin and stdout.

- Supports only `textDocument/formatting` requests
- Synchronizes whole documents only
- Searches for the config file from the document path and re-reads it for each request
- Does not format a document with syntax errors, and returns no edits for it
- Exits with code 1 if it stops without receiving a shutdown request

## `nuten completions`

```nushell
nuten completions <SHELL>
```

Outputs the shell completion script to stdout.

| Argument | Description              | Values                                              |
| -------- | ------------------------ | --------------------------------------------------- |
| `SHELL`  | Specify the target shell | `bash`, `elvish`, `fish`, `nu`, `powershell`, `zsh` |

```nushell
nuten completions nu | save nuten-completions.nu
```
