---
title: Prettier
---

# Prettier

## インストール

> [!NOTE]
> プラグインは、まだ npm と JSR に公開していません。

Nuten は Prettier のプラグインとして使えます。プラグインは `nuten` コマンドを呼び出さないので、`nuten` をインストールする必要はありません。

::: code-group

```nushell [Vite+]
vp i --dev @nuten/prettier-plugin
```

```nushell [npm]
npm install --save-dev @nuten/prettier-plugin
```

```nushell [yarn]
yarn add --dev @nuten/prettier-plugin
```

```nushell [pnpm]
pnpm add --save-dev @nuten/prettier-plugin
```

```nushell [Deno]
deno add --dev jsr:@nuten/prettier-plugin
```

```nushell [bun]
bun add --dev @nuten/prettier-plugin
```

:::

インストールしたら、Prettier の設定にプラグインを追加します。

```json [.prettierrc]
{
  "plugins": ["@nuten/prettier-plugin"]
}
```

これで Prettier が、`.nu` と `.nuon` のファイル、shebang で `nu` を指定したスクリプト、Markdown の中の `nu` と `nushell` のコードブロックを整形するようになります。

## 設定

整形の設定は、`.prettierrc` などの Prettier の設定ファイルに書きます。Prettier から使うときは、Nuten の [設定ファイル](/ja/guide/configuration) は読み込まれません。

Prettier の各項目は、Nuten の設定ファイルの項目と次のように対応します。それぞれの意味は [設定項目](/ja/reference/settings) を参照してください。

| Prettier の項目 | 設定ファイルの項目 | 値                                      | 既定値     |
| --------------- | ------------------ | --------------------------------------- | ---------- |
| `printWidth`    | `line_width`       | 同じ                                    | `100`      |
| `useTabs`       | `indent_style`     | `true` で `"tab"`、`false` で `"space"` | `false`    |
| `tabWidth`      | `indent_width`     | 同じ                                    | `4`        |
| `endOfLine`     | `line_ending`      | 同じ。ただし `"cr"` はエラーになる      | `"lf"`     |
| `nutenCommas`   | `commas`           | 同じ                                    | `"inline"` |

Markdown の中のコードブロックでは、`printWidth` と `tabWidth` の既定値は Prettier のものになります。
