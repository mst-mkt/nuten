import { IndentContext, getIndentation, indentUnit } from "@codemirror/language";
import { EditorState } from "@codemirror/state";
import { describe, expect, test } from "vite-plus/test";

import { highlights } from "./highlights";
import { indentation } from "./indentation";

const stateOf = (doc: string) => {
  const extensions = [highlights, indentation, indentUnit.of("    ")];
  return EditorState.create({ doc, extensions });
};

const indentAfterBreak = (state: EditorState, pos: number) => {
  const options = { simulateBreak: pos };
  const context = new IndentContext(state, options);
  return getIndentation(context, pos);
};

const indentBetweenBreaks = (state: EditorState, pos: number) => {
  const options = { simulateBreak: pos, simulateDoubleBreak: true };
  const context = new IndentContext(state, options);
  return getIndentation(context, pos);
};

describe("indentation", () => {
  test("line_after_curly_bracket_is_indented_one_level_deeper", () => {
    const state = stateOf("ls | each {");

    const indent = indentAfterBreak(state, 11);

    expect(indent).toBe(4);
  });

  test("line_after_parenthesis_is_indented_one_level_deeper", () => {
    const state = stateOf("ls (");

    const indent = indentAfterBreak(state, 4);

    expect(indent).toBe(4);
  });

  test("line_after_bracket_is_indented_one_level_deeper_than_its_line", () => {
    const state = stateOf("  [");

    const indent = indentAfterBreak(state, 3);

    expect(indent).toBe(6);
  });

  test("line_after_bracket_and_spaces_is_indented_one_level_deeper", () => {
    const state = stateOf("ls | each { ");

    const indent = indentAfterBreak(state, 12);

    expect(indent).toBe(4);
  });

  test("line_between_brackets_is_indented_one_level_deeper", () => {
    const state = stateOf("  {}");

    const indent = indentBetweenBreaks(state, 3);

    expect(indent).toBe(6);
  });

  test("line_after_bracket_in_string_is_left_to_editor", () => {
    const state = stateOf('  echo "(');

    const indent = indentAfterBreak(state, 9);

    expect(indent).toBeNull();
  });

  test("line_after_bracket_in_comment_is_left_to_editor", () => {
    const state = stateOf("  # {");

    const indent = indentAfterBreak(state, 5);

    expect(indent).toBeNull();
  });

  test("line_after_other_text_is_left_to_editor", () => {
    const state = stateOf("  ls");

    const indent = indentAfterBreak(state, 4);

    expect(indent).toBeNull();
  });
});
