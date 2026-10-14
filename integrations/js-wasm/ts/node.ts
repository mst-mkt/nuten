import { readFileSync } from "node:fs";

import { readDefaultOptions } from "./defaults";
import type { RequiredOptions } from "./format";
import { initSync } from "./gen/nuten.js";

initSync({ module: readFileSync(new URL("./gen/nuten_bg.wasm", import.meta.url)) });

export const defaultOptions: Readonly<RequiredOptions> = readDefaultOptions();
export * from "./format";
