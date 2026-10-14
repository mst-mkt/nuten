import { readDefaultOptions } from "./defaults";
import type { RequiredOptions } from "./format";
import { initSync } from "./gen/nuten.js";
import module from "./gen/nuten_bg.wasm";

initSync({ module });

export const defaultOptions: Readonly<RequiredOptions> = readDefaultOptions();
export * from "./format";
