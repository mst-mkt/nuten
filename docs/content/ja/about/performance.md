---
title: 性能
---

# 性能

Nuten と [nufmt](https://github.com/nushell/nufmt) を、実際のスクリプトを集めた [corpus](/ja/about/correctness#corpus) で比べています。

```benchmark
overview
```

## ディレクトリの整形

`nuten format --list-different` と `nufmt --dry-run` で、ディレクトリ内のすべてのファイルを整形し、変わるファイルを報告するまでの時間を比べます。値は計測した回の中央値です。Nuten も nufmt もファイルを並列に処理するので、全 CPU と 1 CPU で動かしたときの時間を並べています。

条件を揃えるため、計測には両方のフォーマッタが失敗せず AST も変えずに整形できたファイルだけを使います。対象外のファイルは [比較から外したファイル](#比較から外したファイル) にまとめています。

### 全体

```benchmark
directory all
```

### nu-std

```benchmark
directory nu-std
```

## 単一ファイルの整形

stdin から単一ファイルを整形したときに CPU が実行した命令数の中央値です。エディタや言語サーバーは、この方法で整形します。

単一ファイルの整形は一瞬で終わるので、時間を測るとプロセスの起動にかかる時間がほとんどを占めます。そこで、マシンの速さや混み具合に左右されない命令数を valgrind で数えています。

```benchmark
single
```

## メモリ

実行中に使ったメモリの最大量です。単一ファイルは、ファイルを一つずつ整形したときの中央値です。全体は、ディレクトリ内のすべてのファイルを全 CPU で整形したときの値です。

```benchmark
memory
```

## バイナリサイズ

インストールしたときの実行ファイルの大きさです。

```benchmark
size
```

## 比較から外したファイル

比べるのは、両方のフォーマッタが整形でき、その出力を Nushell が入力と同じ AST に解析するファイルだけです。それ以外のファイルは、どちらのフォーマッタについても比較から外しています。冒頭の「正しさ」は、このうち AST を変えるために外したファイルの数です。

```benchmark
excluded
```

## 条件

```benchmark
conditions
```

## 手元で再現する

ベンチマークは [`tasks/benchmark`](https://github.com/mst-mkt/nuten/tree/main/tasks/benchmark) にあります。`nix/dev` の devShell に hyperfine, valgrind, corpus が揃っていて、nufmt は `nix build ./nix/dev#nufmt` でビルドできます。devShell の Nushell で次を実行すると計測できます。

```nushell
nu tasks/benchmark/main.nu nufmt $env.NUTEN_CORPUS path/to/nuten path/to/nufmt path/to/nuten-oracle --runs 10 | to json | save -f compare.json
```
