---
title: JavaScript
---

# JavaScript

## インストール

> [!NOTE]
> パッケージは、まだ npm と JSR に公開していません。

`@nuten/format` を使うと、JavaScript から Nushell を整形できます。WebAssembly で動くので、Node.js, Deno, Bun だけでなく、ブラウザや Cloudflare Workers でも使えます。`nuten` コマンドは呼び出さないので、`nuten` をインストールする必要はありません。

::: code-group

```nushell [Vite+]
vp i @nuten/format
```

```nushell [npm]
npm install @nuten/format
```

```nushell [yarn]
yarn add @nuten/format
```

```nushell [pnpm]
pnpm add @nuten/format
```

```nushell [Deno]
deno add jsr:@nuten/format
```

```nushell [bun]
bun add @nuten/format
```

:::

## 使い方

`format` は Nushell のコードを整形して返します。オプションは 2 番目の引数で渡します。

```ts
import { format } from "@nuten/format";

format("ls|sort"); // "ls | sort\n"
format("[1, 2, 3]", { commas: "none" }); // "[1 2 3]\n"
```

コードに構文エラーがあると、`format` は `FormatError` を投げます。`FormatError` の `errors` には、各エラーの行, 列, メッセージが入っています。行と列は 1 から数えます。列は UTF-16 のコードユニットではなく、コードポイントで数えます。

```ts
import { FormatError, format } from "@nuten/format";

try {
  format("ls && echo");
} catch (error) {
  if (error instanceof FormatError) {
    error.errors; // [{ line: 1, column: 4, message: "`&&` is not an operator; ..." }]
  }
}
```

存在しないオプションや不正な値を渡すと、`format` は `TypeError` を投げます。

## オプション

各オプションは、設定ファイルの項目と次のように対応します。それぞれの意味は [設定項目](/ja/reference/settings) を参照してください。このパッケージは Nuten の [設定ファイル](/ja/guide/configuration) を読み込みません。

| オプション    | 設定ファイルの項目 | 型                                             | 既定値     |
| ------------- | ------------------ | ---------------------------------------------- | ---------- |
| `lineWidth`   | `line_width`       | `number`                                       | `100`      |
| `indentStyle` | `indent_style`     | `"space" \| "tab"`                             | `"space"`  |
| `indentWidth` | `indent_width`     | `number`                                       | `4`        |
| `lineEnding`  | `line_ending`      | `"lf" \| "crlf" \| "auto"`                     | `"auto"`   |
| `commas`      | `commas`           | `"none" \| "inline" \| "always" \| "trailing"` | `"inline"` |

省略したオプションには既定値が使われます。既定値は `defaultOptions` からも参照できます。
