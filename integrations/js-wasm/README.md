# @nuten/wasm

[Nuten](https://nuten.keito.dev), a formatter for Nushell, built into Wasm.
It formats Nushell in browsers, Node.js, and Cloudflare Workers without the `nuten` binary.

## Installation

```nushell
pnpm add @nuten/wasm
```

```nushell
deno add jsr:@nuten/wasm
```

## Usage

```ts
import { format } from "@nuten/wasm";

format("ls|sort"); // "ls | sort\n"
format("[1, 2, 3]", { commas: "none" }); // "[1 2 3]\n"
```

A syntax error throws a `FormatError`, whose `errors` list where each error is.
Lines and columns count from 1, and columns count code points.

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

## Options

Each option is a [setting](https://nuten.keito.dev/reference/settings) of the config file.

| Option        | Setting        | Type                                           | Default    |
| ------------- | -------------- | ---------------------------------------------- | ---------- |
| `lineWidth`   | `line_width`   | `number`                                       | `100`      |
| `indentStyle` | `indent_style` | `"space" \| "tab"`                             | `"space"`  |
| `indentWidth` | `indent_width` | `number`                                       | `4`        |
| `lineEnding`  | `line_ending`  | `"lf" \| "crlf" \| "auto"`                     | `"auto"`   |
| `commas`      | `commas`       | `"none" \| "inline" \| "always" \| "trailing"` | `"inline"` |

## License

[MIT](https://github.com/mst-mkt/nuten/blob/main/LICENSE)
