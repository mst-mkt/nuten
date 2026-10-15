import type { RequiredOptions } from "@nuten/wasm";

export const indentStyleLabels = {
  space: "Spaces",
  tab: "Tabs",
} satisfies Record<RequiredOptions["indentStyle"], string>;

export const lineEndingLabels = {
  auto: "Auto",
  lf: "LF",
  crlf: "CRLF",
} satisfies Record<RequiredOptions["lineEnding"], string>;

export const commasLabels = {
  none: "None",
  inline: "Inline",
  always: "Always",
  trailing: "Trailing",
} satisfies Record<RequiredOptions["commas"], string>;
