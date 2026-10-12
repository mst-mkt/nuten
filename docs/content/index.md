---
layout: entry
title: Nuten
description: A formatter for Nushell. Robust, fast, and lightweight.
hero:
  name: Nuten
  text: A formatter for Nushell
  tagline: Robust, fast, and lightweight.
  image:
    src: logo.svg
    alt: Nuten logo
    width: 220
    height: 220
  actions:
    - theme: brand
      text: Get Started
      link: /guide/installation
    - theme: alt
      text: View on GitHub
      link: https://github.com/mst-mkt/nuten
features:
  - icon: "lucide:zap"
    title: Fast and lightweight
    details: Written in Rust, faster than nufmt, and lighter on memory and disk.
  - icon: "lucide:shield-check"
    title: Never break
    details: Keeps every token and comment, and skips a file it cannot parse.
  - icon: "lucide:scan-eye"
    title: Read correctly
    details: Parses code the way nu does, and is checked against nu itself.
---

::: compare

```nu [Before]
# Pick a branch in a TUI and switch to it
def "git pick" [--remote(-r)] {
let flags = if $remote {[--remotes]} else {[]}
let picked = ^git branch --format "%(refname:short)%09%(upstream:short)" ...$flags|lines|parse "{name}\t{upstream}"
  |tui label --title "branches"|tui search --placeholder "filter" --bind /
    | tui split --sizes [40% 1fr] [(tui table --columns [name upstream])
  (tui preview {|row| ^git log --oneline -n 20 $row.name})]
    |tui label --status "enter: switch  q: quit"|tui run

match $picked {
{action: quit} => {print "Nothing picked"}
{selected: {name: $name}} if ($name|str starts-with "origin/") => {
^git switch --track $name}
  {selected: {name: $name}} => {^git switch $name}
}
}
```

```nu [After]
# Pick a branch in a TUI and switch to it
def "git pick" [--remote(-r)] {
    let flags = if $remote { [--remotes] } else { [] }
    let picked = ^git branch --format "%(refname:short)%09%(upstream:short)" ...$flags
        | lines
        | parse "{name}\t{upstream}"
        | tui label --title "branches"
        | tui search --placeholder "filter" --bind /
        | tui split --sizes [40%, 1fr] [
              (tui table --columns [name, upstream])
              (tui preview {|row| ^git log --oneline -n 20 $row.name })
          ]
        | tui label --status "enter: switch  q: quit"
        | tui run

    match $picked {
        { action: quit } => { print "Nothing picked" }
        { selected: { name: $name } } if ($name | str starts-with "origin/") => {
            ^git switch --track $name
        }
        { selected: { name: $name } } => { ^git switch $name }
    }
}
```

:::

## Install

::: code-group

```sh [Cargo]
cargo install --git https://github.com/mst-mkt/nuten nuten
```

```sh [Nix]
nix profile install github:mst-mkt/nuten
```

:::

## How Nuten stays correct

Every change has to pass several layers of tests.

::: card-grid
::: card

### Parser snapshots

Parses over 560 hand-written inputs and pins the result. No parser change goes unnoticed.
:::
::: card

### Formatter fixtures

Formats over 600 inputs and compares them with the expected output. The style stays as decided.
:::
::: card

### Real-world corpus

Formats nearly 1,800 real scripts. No token or comment is lost.
:::
::: card

### nu as oracle

Asks nu to parse each script before and after formatting. The meaning never changes.
:::
::: card

### Fuzzing

Feeds random input to the lexer, parser, and formatter. Nothing crashes, nothing is lost.
:::
::: card

### CLI integration

Runs the built binary on files, stdin, and configs. Exit codes and file handling behave as intended.
:::
:::

## Editors and tools

The same formatter, from the CLI to the tools you already use.

::: tools

- <span class="icon-[simple-icons--visualstudiocode]"></span> VS Code
- <span class="icon-[simple-icons--zedindustries]"></span> Zed
- LSP
- dprint
- treefmt
- <span class="icon-[simple-icons--githubactions]"></span> GitHub Actions
- <span class="icon-[simple-icons--gitlab]"></span> GitLab CI

:::
