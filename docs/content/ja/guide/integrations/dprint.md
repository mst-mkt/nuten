---
title: dprint
---

# dprint

## インストール

> [!NOTE]
> プラグインは、まだ GitHub のリリースに含めておらず、[dprint のプラグイン一覧](https://dprint.dev/plugins/) にも載せていません。

Nuten は dprint の Wasm プラグインとして使えます。プラグインに Nuten の整形処理が含まれているので、`nuten` のインストールは要りません。

次のコマンドで、プラグインを `dprint.json` に追加します。

::: code-group

```nushell [最新版]
dprint config add mst-mkt/nuten
```

```nushell [バージョンを指定]
dprint config add https://plugins.dprint.dev/mst-mkt/nuten-{version}.wasm
```

:::

## 設定

`dprint.json` の `"nuten"` に設定を書きます。dprint から使うときは、プロジェクトの [設定ファイル](/ja/guide/configuration) は使われません。

```json [dprint.json]
{
  "nuten": {
    "lineWidth": 80,
    "indentWidth": 2
  }
}
```

項目名は dprint の他のプラグインに合わせてあり、設定ファイルとは次のように対応します。各項目の意味は [設定項目](/ja/reference/settings) を参照してください。

| dprint の項目 | 設定ファイルの項目 | 値                                      |
| ------------- | ------------------ | --------------------------------------- |
| `lineWidth`   | `line_width`       | 同じ                                    |
| `useTabs`     | `indent_style`     | `true` で `"tab"`、`false` で `"space"` |
| `indentWidth` | `indent_width`     | 同じ                                    |
| `newLineKind` | `line_ending`      | 同じ                                    |
| `commas`      | `commas`           | 同じ                                    |

`lineWidth`, `useTabs`, `indentWidth`, `newLineKind` は、`"nuten"` に書かなければ、`dprint.json` の一番上に書いた dprint 全体の設定が使われます。

整形するのは `.nu` と `.nuon` のファイルです。対象から外すファイルは、dprint の `excludes` で指定します。
