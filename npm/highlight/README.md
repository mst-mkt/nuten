# @nuten/highlight

Syntax highlighting of Nushell, built into Wasm from [nuten-highlight](https://github.com/mst-mkt/nuten/tree/main/crates/nuten_highlight).
It highlights Nushell in browsers and Node.js, and only the [playground](https://nuten.keito.dev/playground) uses it for now.

## Usage

```ts
import { highlight } from "@nuten/highlight";

highlight("ls -la"); // [{ start: 0, end: 2, type: "function" }, { start: 3, end: 6, type: "flag" }]
```

The highlights come in order and never overlap.
`start` and `end` are indices into the string, so they count UTF-16 code units.
The types are the semantic token types of LSP, and `flag`.

## License

[MIT](https://github.com/mst-mkt/nuten/blob/main/LICENSE)
