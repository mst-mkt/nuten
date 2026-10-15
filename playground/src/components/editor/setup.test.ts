import { IndentContext, getIndentation, indentUnit } from "@codemirror/language";
import { EditorState } from "@codemirror/state";
import { describe, expect, test } from "vite-plus/test";

import { highlights } from "./highlights";
import { editableSetup } from "./setup";

const stateOf = (doc: string) => {
  const extensions = [highlights, editableSetup, indentUnit.of("    ")];
  return EditorState.create({ doc, extensions });
};

describe("editable setup", () => {
  test("editable_setup_indents_inside_brackets", () => {
    const state = stateOf("ls {");
    const context = new IndentContext(state, { simulateBreak: 4 });

    const indent = getIndentation(context, 4);

    expect(indent).toBe(4);
  });

  test("editable_setup_knows_line_comments", () => {
    const state = stateOf("ls");

    const tokens = state.languageDataAt("commentTokens", 0);

    expect(tokens).toEqual([{ line: "#" }]);
  });
});
