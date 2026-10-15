---
layout: entry
lastUpdated: false
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
      text: Try in Playground
      link: https://nuten.keito.dev/playground/
features:
  - icon: "lucide:zap"
    title: Fast and lightweight
    link: /about/performance
    details: Faster than nufmt and lighter on memory and disk, written in Rust.
  - icon: "lucide:shield-check"
    title: Never break
    link: /about/correctness
    details: Keeps every token and comment, and skips any file with syntax errors.
  - icon: "lucide:scan-text"
    title: Read correctly
    link: /about/correctness#oracle
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

::: card-grid

::: link-card {/about/correctness#parsing-and-formatting} Lossless CST
Parses code into a tree that keeps everything, including whitespace and comments.
:::

::: link-card {/about/correctness#snapshots} Parser snapshots
Catches unexpected changes in parsing results.
:::

::: link-card {/about/correctness#fixtures} Formatter fixtures
Compares formatted output with the expected output, so the style never drifts.
:::

::: link-card {/about/correctness#corpus} Real-world corpus
Formats real-world scripts and makes sure no token or comment is lost.
:::

::: link-card {/about/correctness#oracle} nu as oracle
Asks nu whether it parses code the same way before and after formatting.
:::

::: link-card {/about/correctness#fuzzing} Fuzzing
Throws random input at Nuten to make sure it never crashes or loses code.
:::

:::

## Editors and tools

The same formatter, from the CLI to the tools you already use.

::: tools

- [<span class="icon-[simple-icons--visualstudiocode]"></span> VS Code](/guide/integrations/vscode)
- [<span class="icon-[simple-icons--zedindustries]"></span> Zed](/guide/integrations/zed)
- [LSP](/reference/cli#nuten-lsp)
- [dprint](/guide/integrations/dprint)
- [<span class="icon-[simple-icons--prettier]"></span> Prettier](/guide/integrations/prettier)
- [treefmt](/guide/integrations/treefmt)
- [<span class="icon-[simple-icons--githubactions]"></span> GitHub Actions](/guide/integrations/ci#github-actions)
- [<span class="icon-[simple-icons--gitlab]"></span> GitLab CI](/guide/integrations/ci#gitlab-ci)

:::
