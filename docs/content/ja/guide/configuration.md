---
title: 設定
---

# 設定

## 設定ファイル

以下のいずれかの名前で置きます。同じディレクトリに複数ある場合は、`.nuon`, `.jsonc`, `.json` の順に優先します。

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

項目の一覧は [設定項目](/ja/reference/settings) を参照

## 設定ファイルの探索

- 整形するファイルのディレクトリから親へ順にたどり、最初に見つかった設定ファイルを使う
- `.git` などがあるディレクトリまでで止まる
- 見つからなければ各項目の既定値を使う

```nushell
nuten format --config ./ci/nuten.nuon
```

`--config` で指定した場合は探索せず、その設定を使います。

[treefmt-nix](/ja/guide/integrations/treefmt#treefmt-nix) から使う場合は、モジュールの `settings` に書くこともできます。 \
[dprint](/ja/guide/integrations/dprint) から使う場合は、設定ファイルではなく `dprint.json` に書きます。 \
[Prettier](/ja/guide/integrations/prettier) から使う場合は、Prettier の設定に書きます。

## 対象のファイル

```nu [nuten.nuon]
{
    include: ["*.nu", "*.nuon"]
    exclude: ["vendor", "scripts/generated/*.nu"]
    respect_gitignore: true
}
```

- `include`: ディレクトリを渡したときに対象にするファイル
- `exclude`: 対象から外すファイルやディレクトリ。ファイルを直接渡した場合にも効く
- `respect_gitignore`: `.gitignore` などで無視されたファイルを対象から外すか

## 設定の上書き

```nu [nuten.nuon]
{
    line_width: 100
    overrides: [
        { include: ["tests/**/*.nu"], line_width: 120 }
    ]
}
```

各要素は、`include` と `exclude` に一致したファイルに、それ以外の項目を上書きします。複数の要素に一致した場合は後のものが優先されます。

## EditorConfig

`.editorconfig` がある場合は、その設定も使います。

```ini [.editorconfig]
root = true

[*.nu]
indent_style = space
indent_size = 2
max_line_length = 100
```

`.editorconfig` は次のように探します。

- 整形するファイルのディレクトリから親へ順にたどり、見つかった `.editorconfig` をすべて読む
- `root = true` を持つ `.editorconfig` か、`.git` などがあるディレクトリまでで止まる
- 同じプロパティが複数の `.editorconfig` にあれば、整形するファイルに近いものを使う

読むプロパティは次のとおりです。表にないプロパティと、nuten が受け付けない値は無視します。

| EditorConfig      | nuten          | 備考                                         |
| ----------------- | -------------- | -------------------------------------------- |
| `indent_style`    | `indent_style` |                                              |
| `indent_size`     | `indent_width` | スペースで字下げする場合                     |
| `tab_width`       | `indent_width` | タブで字下げする場合。なければ `indent_size` |
| `end_of_line`     | `line_ending`  |                                              |
| `max_line_length` | `line_width`   | `off` なら幅で行を折らない                   |

設定ファイルとその `overrides` に書いた項目は、`.editorconfig` より優先されます。`.editorconfig` を読まないようにするには、[`editorconfig`](/ja/reference/settings#editorconfig) を `false` にします。

[dprint](/ja/guide/integrations/dprint) と [Prettier](/ja/guide/integrations/prettier) から使う場合、nuten は `.editorconfig` を読みません。Prettier は `.editorconfig` を自分で読み、Prettier の設定として nuten に渡します。

## JSON Schema

設定ファイルの JSON Schema は `nuten schema` で出力できます。

```nushell
nuten schema | save nuten.schema.json
```

JSON や JSONC で書く場合は、`$schema` に指定するとエディタが項目を補完します。

```jsonc [nuten.jsonc]
{
  "$schema": "./nuten.schema.json",
  "line_width": 100,
}
```
