import type { RequiredOptions } from "@nuten/format";
import { describe, expect, test } from "vite-plus/test";

import { summaryOf } from "./summary";

const defaults: RequiredOptions = {
  lineWidth: 100,
  indentStyle: "space",
  indentWidth: 4,
  lineEnding: "auto",
  commas: "inline",
};

describe("summary", () => {
  test("summary_names_each_setting_in_words", () => {
    expect(summaryOf(defaults)).toBe("100 columns · 4 spaces · Auto · Inline commas");
  });

  test("summary_of_tab_indent_leaves_out_indent_width", () => {
    expect(summaryOf({ ...defaults, indentStyle: "tab", indentWidth: 2 })).toBe(
      "100 columns · Tabs · Auto · Inline commas",
    );
  });

  test("summary_counts_one_in_singular", () => {
    expect(summaryOf({ ...defaults, lineWidth: 1, indentWidth: 1 })).toBe(
      "1 column · 1 space · Auto · Inline commas",
    );
  });

  test("summary_names_every_choice_of_line_ending_and_commas", () => {
    expect(summaryOf({ ...defaults, lineEnding: "crlf", commas: "none" })).toBe(
      "100 columns · 4 spaces · CRLF · No commas",
    );
    expect(summaryOf({ ...defaults, lineEnding: "lf", commas: "trailing" })).toBe(
      "100 columns · 4 spaces · LF · Trailing commas",
    );
    expect(summaryOf({ ...defaults, commas: "always" })).toBe(
      "100 columns · 4 spaces · Auto · Commas always",
    );
  });
});
