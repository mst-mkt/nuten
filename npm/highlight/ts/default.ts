import init from "./gen/highlight.js";

const wasm = new URL("./gen/highlight_bg.wasm", import.meta.url);

await init({
  module_or_path:
    wasm.protocol === "file:"
      ? globalThis.process.getBuiltinModule("node:fs").readFileSync(wasm)
      : wasm,
});

export * from "./highlight";
