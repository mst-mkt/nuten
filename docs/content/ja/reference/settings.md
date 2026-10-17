---
title: 設定項目
---

# 設定項目

## line_width

**Type** `integer` \
**Default** `100`

1 行の最大の桁数を指定します。超える行は、折り返せる箇所で折り返します。

## indent_style

**Type** `"space" | "tab"` \
**Default** `"space"`

インデントに使う文字を指定します。

## indent_width

**Type** `integer` \
**Default** `4` \
**Range** 1 ~ 255

インデント 1 段の桁数を指定します。

タブでインデントするときは、タブ 1 つをこの桁数として行の長さを数えます。

## line_ending

**Type** `"lf" | "crlf" | "auto"` \
**Default** `"auto"`

改行コードを指定します。`"auto"` では、ファイル中の最初の改行に合わせます。

文字列や属性のブロックの中の改行は、この設定に関わらず書いたまま残します。

## commas

**Type** `"none" | "inline" | "always" | "trailing"` \
**Default** `"inline"`

リストやレコードなどで、項目を区切るコンマの付け方を指定します。

::: compare

```nushell [none]
[1 2 3]

[
    "src/main.nu"
    "tests/main.nu"
]
```

```nushell [inline]
[1, 2, 3]

[
    "src/main.nu"
    "tests/main.nu"
]
```

```nushell [always]
[1, 2, 3]

[
    "src/main.nu",
    "tests/main.nu"
]
```

```nushell [trailing]
[1, 2, 3]

[
    "src/main.nu",
    "tests/main.nu",
]
```

:::

## include

**Type** `string[]` \
**Default** `["*.nu", "*.nuon"]`

整形するファイルを glob パターンで指定します。直接渡したファイルは、一致しなくても整形します。

パターンは設定ファイルのあるディレクトリを基準にします。`/` を含まないパターンは、どの階層のファイル名にも一致します。

## exclude

**Type** `string[]` \
**Default** `[]`

整形しないファイルやディレクトリを glob パターンで指定します。直接渡したファイルにも効きます。

パターンは設定ファイルのあるディレクトリを基準にします。`/` を含まないパターンは、どの階層のファイル名やディレクトリ名にも一致します。

## respect_gitignore

**Type** `boolean` \
**Default** `true`

`.gitignore` などに書かれたファイルを、対象から外すかどうかを指定します。

## editorconfig

**Type** `boolean` \
**Default** `true`

`.editorconfig` から設定を読むかどうかを指定します。読むプロパティと優先順は、[設定](/ja/guide/configuration#editorconfig) の EditorConfig の節にあります。

## overrides

**Type** `{ include: string[], exclude?: string[], line_width?, indent_style?, indent_width?, line_ending?, commas? }[]` \
**Default** `[]`

一部のファイルだけ設定を変えます。各要素は次の項目を持ちます。

| 項目                                                                  | 型           | 説明                                         |
| --------------------------------------------------------------------- | ------------ | -------------------------------------------- |
| `include`                                                             | `string[]`   | 対象にするファイルのパターン                 |
| `exclude`                                                             | `string[]`   | 対象から外すファイルやディレクトリのパターン |
| `line_width`, `indent_style`, `indent_width`, `line_ending`, `commas` | 各項目と同じ | 対象のファイルに使う設定                     |

```nu [nuten.nuon]
{
    overrides: [
        { include: ["tests/**/*.nu"], line_width: 120 }
    ]
}
```

1 つのファイルに複数の要素が当てはまるときは、前の要素から順に適用します。同じ設定は後の要素が優先されます。
