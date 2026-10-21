---
title: 整形
---

# 整形

## 対象のファイル

```nushell
nuten format            # カレントディレクトリ以下の .nu と .nuon
nuten format src        # src 以下の .nu と .nuon
nuten format a.nu b.nu  # 指定したファイル
```

ディレクトリを渡した場合、以下のファイルは対象になりません。

- `.gitignore` などで無視されているファイル
- `.git` などの VCS のディレクトリにあるファイル
- シンボリックリンクの先 (リンクはたどらない)

ファイルを直接渡した場合は、拡張子に関わらず整形します。対象の選び方は [設定](/ja/guide/configuration) の `include`, `exclude`, `respect_gitignore` で変えられます。

## 書き換え

ファイルはその場で書き換えるので、ハードリンクやパーミッションはそのまま残ります。書き込みに失敗した場合は元の内容に戻します。

最後に、結果の内訳を stderr に 1 行出します。

```
10 files: 2 formatted, 1 failed, 3 skipped, 4 excluded
```

- `formatted`: 書き換えた
- `unchanged`: 差分がなかった
- `failed`: 構文エラーや読み書きの失敗があった
- `skipped`: 設定ファイルの誤りで処理しなかった
- `excluded`: `exclude` や `.gitignore` で除いた

## 差分を見る

```nushell
nuten format --check
```

ファイルを変更せず、差分を stdout に出します。

```ansi
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

- 赤は整形前、緑は整形後の行。色が付かない環境では `-` と `+`
- `·` は空白、`→` はタブ、`⏎` は増減した改行
- 改行コードだけが変わるときは `line endings: CRLF → LF`

```nushell
nuten format --list-different
```

差分のあるファイルのパスだけを 1 行ずつ出します。スクリプトから使う場合に向いています。

いずれも、差分があれば exit code は 1 になります。CI で結果を報告するには [`--reporter`](/ja/guide/integrations/ci) を使います。

## stdin

```nushell
open --raw a.nu | nuten format -
```

`-` を渡すと stdin を整形し、結果を stdout に出します。設定ファイルはカレントディレクトリを基準に探し、`overrides` と `exclude` は効きません。

内容がどのファイルのものか分かっているときは、`--stdin-filepath` でそのパスを渡します。そのパスを基準に設定ファイルを探し、`overrides` と `exclude` も効きます。

```nushell
$buffer | nuten format --stdin-filepath src/a.nu -
```
