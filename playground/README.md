# @nuten/playground

The playground of [Nuten](https://github.com/mst-mkt/nuten), a formatter for Nushell.
It formats Nushell in the browser through [@nuten/wasm](../integrations/js-wasm), and is built with React, Base UI, and CodeMirror on Vite+.

## Development

The tasks build @nuten/wasm first, which needs the tools that the `nix/dev` devShell provides.

```nushell
pnpm install
vp run @nuten/playground#dev
vp run @nuten/playground#build
```

## Tests

The unit tests in `src` check how the playground reads the result of formatting, such as where each syntax error points in the editor.

```nushell
vp run @nuten/playground#test
```
