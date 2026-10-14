import { initSync } from "./gen/nuten.js";
import module from "./gen/nuten_bg.wasm";

initSync({ module });

export * from "./format";
