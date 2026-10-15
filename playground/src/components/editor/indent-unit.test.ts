import { indentUnit } from "@codemirror/language";
import { EditorState } from "@codemirror/state";
import { describe, expect, test } from "vite-plus/test";

import { indentUnitOf } from "./indent-unit";

describe("indent unit", () => {
  test("indent_unit_is_spaces_of_indent_width", () => {
    const state = EditorState.create({
      extensions: indentUnitOf({ indentStyle: "space", indentWidth: 2 }),
    });

    const unit = state.facet(indentUnit);

    expect(unit).toBe("  ");
  });

  test("indent_unit_is_tab_whatever_indent_width", () => {
    const state = EditorState.create({
      extensions: indentUnitOf({ indentStyle: "tab", indentWidth: 2 }),
    });

    const unit = state.facet(indentUnit);

    expect(unit).toBe("\t");
  });
});
