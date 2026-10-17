import type { Pair } from "./comparison";
import { element } from "./html";

export type BarGroup = {
  name?: string;
  label: string;
  values: Pair;
  format: (value: number) => string;
  note?: string | undefined;
};

const barGroup = ({ name, label, values, format, note }: BarGroup) => {
  const longest = Math.max(values.nuten, values.nufmt);

  const row = (formatter: keyof Pair, rowNote?: string) => {
    const value = values[formatter];
    const width = longest === 0 ? 0 : (value / longest) * 100;
    const fill =
      value === 0
        ? undefined
        : element("span", { class: "benchmark-fill", style: `width: ${width.toFixed(1)}%` });
    const noteElement =
      rowNote === undefined ? undefined : element("span", { class: "benchmark-note" }, rowNote);

    return element(
      "div",
      { class: `benchmark-row benchmark-row--${formatter}` },
      element("span", {}, formatter),
      element("span", { class: "benchmark-track" }, fill, noteElement),
      element("span", {}, format(value)),
    );
  };
  const heading = name === undefined ? label : [element("strong", {}, name), ` · ${label}`];

  return element(
    "div",
    { class: "benchmark-group" },
    element("div", { class: "benchmark-label" }, heading),
    row("nuten", note),
    row("nufmt"),
  );
};

export const barGroups = (groups: BarGroup[], layout?: string) => {
  const modifier = layout === undefined ? "" : ` benchmark-bars--${layout}`;

  return element("div", { class: `benchmark-bars${modifier}` }, groups.map(barGroup));
};
