import { readDefaultOptions } from "./defaults";
import type { RequiredOptions } from "./format";
import init from "./gen/nuten.js";

const wasm = new URL("./gen/nuten_bg.wasm", import.meta.url);

await init({
  module_or_path:
    wasm.protocol === "file:"
      ? globalThis.process.getBuiltinModule("node:fs").readFileSync(wasm)
      : wasm,
});

export const defaultOptions: Readonly<RequiredOptions> = readDefaultOptions();
export * from "./format";
