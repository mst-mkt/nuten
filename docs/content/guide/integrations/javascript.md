---
title: JavaScript
---

# JavaScript

## Installation

> [!NOTE]
> The package is not yet published on npm or JSR.

`@nuten/wasm` lets you format Nushell from JavaScript. Since it runs on WebAssembly, it works in browsers and Cloudflare Workers as well as in Node.js, Deno, and Bun. It does not call the `nuten` command, so you do not need to install `nuten`.

::: code-group

```nushell [Vite+]
vp i @nuten/wasm
```

```nushell [npm]
npm install @nuten/wasm
```

```nushell [yarn]
yarn add @nuten/wasm
```

```nushell [pnpm]
pnpm add @nuten/wasm
```

```nushell [Deno]
deno add jsr:@nuten/wasm
```

```nushell [bun]
bun add @nuten/wasm
```

:::

## Usage

`format` takes Nushell code and returns it formatted. Options go in the second argument.

```ts
import { format } from "@nuten/wasm";

format("ls|sort"); // "ls | sort\n"
format("[1, 2, 3]", { commas: "none" }); // "[1 2 3]\n"
```

If the code has syntax errors, `format` throws a `FormatError`. Its `errors` property lists the line, column, and message of each error. Lines and columns start at 1, and columns count code points, not UTF-16 code units.

```ts
import { FormatError, format } from "@nuten/wasm";

try {
  format("ls && echo");
} catch (error) {
  if (error instanceof FormatError) {
    error.errors; // [{ line: 1, column: 4, message: "`&&` is not an operator; ..." }]
  }
}
```

If an option is unknown or has an invalid value, `format` throws a `TypeError`.

## Options

The options map to the config file as follows. See [Settings](/reference/settings) for what each setting does. The package does not read the project's [config file](/guide/configuration).

| Option        | Config file    | Type                                           | Default    |
| ------------- | -------------- | ---------------------------------------------- | ---------- |
| `lineWidth`   | `line_width`   | `number`                                       | `100`      |
| `indentStyle` | `indent_style` | `"space" \| "tab"`                             | `"space"`  |
| `indentWidth` | `indent_width` | `number`                                       | `4`        |
| `lineEnding`  | `line_ending`  | `"lf" \| "crlf" \| "auto"`                     | `"auto"`   |
| `commas`      | `commas`       | `"none" \| "inline" \| "always" \| "trailing"` | `"inline"` |

Options left out take their defaults, which are also exported as `defaultOptions`.
