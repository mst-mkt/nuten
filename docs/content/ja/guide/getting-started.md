---
title: はじめる
---

# はじめる

## ファイルを整形する

```nushell
nuten format
```

カレントディレクトリ以下の `.nu` と `.nuon` をすべて整形します。対象を絞るには、ファイルかディレクトリを渡します。

```nushell
nuten format src scripts/build.nu
```

## 整形せずに差分を見る

```nushell
nuten format --check
```

差分がある場合、以下のように表示されます。

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

## 次のステップ

- [設定](/ja/guide/usage/configuration)
- [エディタなどの連携](/ja/guide/integrations)
- [CLI リファレンス](/ja/reference/cli)
