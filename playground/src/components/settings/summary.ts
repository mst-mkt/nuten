import type { RequiredOptions } from "@nuten/wasm";

import { indentStyleLabels, lineEndingLabels } from "./labels";

const plural = new Intl.PluralRules("en");

const countOf = (count: number, one: string, other: string) => {
  return `${count} ${plural.select(count) === "one" ? one : other}`;
};

const commasPhrases = {
  none: "No commas",
  inline: "Inline commas",
  always: "Commas always",
  trailing: "Trailing commas",
} satisfies Record<RequiredOptions["commas"], string>;

export const summaryOf = (options: RequiredOptions) => {
  return [
    countOf(options.lineWidth, "column", "columns"),
    options.indentStyle === "tab"
      ? indentStyleLabels.tab
      : countOf(options.indentWidth, "space", "spaces"),
    lineEndingLabels[options.lineEnding],
    commasPhrases[options.commas],
  ].join(" · ");
};
