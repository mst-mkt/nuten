import { readFileSync } from "node:fs";

import { initSync } from "./gen/nuten.js";

initSync({ module: readFileSync(new URL("./gen/nuten_bg.wasm", import.meta.url)) });

export * from "./format";
