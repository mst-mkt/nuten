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
    alt: Nuten のロゴ
    width: 220
    height: 220
  actions:
    - theme: brand
      text: はじめる
      link: /ja/guide/installation
    - theme: alt
      text: GitHub で見る
      link: https://github.com/mst-mkt/nuten
features:
  - icon: "lucide:rocket"
    title: Fast and lightweight
    link: /ja/about/performance
    details: nufmt より速く、メモリやディスクの消費も少ない、Rust 製のフォーマッタです。
  - icon: "lucide:shield-check"
    title: Never break
    link: /ja/about/correctness
    details: トークンとコメントをすべて保ち、エラーのあるファイルには手を加えません。
  - icon: "lucide:scan-text"
    title: Read correctly
    link: /ja/about/correctness#oracle
    details: Nushell と同じようにコードを読み、その結果を Nushell 自身と比べて確かめています。
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

## インストール

::: code-group

```sh [Cargo]
cargo install --git https://github.com/mst-mkt/nuten nuten
```

```sh [Nix]
nix profile install github:mst-mkt/nuten
```

:::

## 正しさを保つ仕組み

::: card-grid

::: link-card {/ja/about/correctness#解析と整形} Lossless CST
空白やコメントも含め、元のコードをすべて保つ構文木に解析します。
:::

::: link-card {/ja/about/correctness#snapshot} Parser snapshots
解析の結果が意図せず変わっていないかを確かめます。
:::

::: link-card {/ja/about/correctness#fixtures} Formatter fixtures
整形の結果が、決めたスタイルになっているかを確かめます。
:::

::: link-card {/ja/about/correctness#corpus} Real-world corpus
実際のスクリプトを整形し、トークンやコメントが消えないかを確かめます。
:::

::: link-card {/ja/about/correctness#oracle} nu as oracle
整形の前後で、Nushell が同じようにコードを読むかを確かめます。
:::

::: link-card {/ja/about/correctness#fuzzing} Fuzzing
ランダムに作った大量の入力で、異常終了やコードの欠落がないかを確かめます。
:::

:::

## エディタとツール

CLI から普段使っているツールまで、同じフォーマッタで整形できます。

::: tools

- [<span class="icon-[simple-icons--visualstudiocode]"></span> VS Code](/ja/guide/integrations/vscode)
- [<span class="icon-[simple-icons--zedindustries]"></span> Zed](/ja/guide/integrations/zed)
- [LSP](/ja/reference/cli#nuten-lsp)
- [dprint](/ja/guide/integrations/dprint)
- [treefmt](/ja/guide/integrations/treefmt)
- [<span class="icon-[simple-icons--githubactions]"></span> GitHub Actions](/ja/guide/integrations/ci#github-actions)
- [<span class="icon-[simple-icons--gitlab]"></span> GitLab CI](/ja/guide/integrations/ci#gitlab-ci)

:::
