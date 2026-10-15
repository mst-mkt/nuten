import type { RequiredOptions } from "./format";
import { defaultOptions as defaultOptionsInWasm } from "./gen/nuten.js";

export const readDefaultOptions = (): Readonly<RequiredOptions> => {
  const defaults = defaultOptionsInWasm();
  try {
    return Object.freeze({
      lineWidth: defaults.lineWidth,
      indentStyle: defaults.indentStyle,
      indentWidth: defaults.indentWidth,
      lineEnding: defaults.lineEnding,
      commas: defaults.commas,
    });
  } finally {
    defaults.free();
  }
};
