import { insertBracket } from "@codemirror/autocomplete";
import { EditorState } from "@codemirror/state";
import { describe, expect, test } from "vite-plus/test";

import { brackets } from "./brackets";
import { highlights } from "./highlights";

const stateAt = (doc: string, cursor: number) => {
  const selection = { anchor: cursor };
  const extensions = [highlights, brackets];
  return EditorState.create({ doc, selection, extensions });
};

const typed = (state: EditorState, bracket: string) => {
  const tr = insertBracket(state, bracket);
  if (tr === null) return null;

  const doc = tr.state.doc.toString();
  const cursor = tr.state.selection.main.head;
  return { doc, cursor };
};

describe("brackets", () => {
  test("parenthesis_is_closed_after_cursor", () => {
    const state = stateAt("ls ", 3);

    const result = typed(state, "(");

    expect(result).toEqual({ doc: "ls ()", cursor: 4 });
  });

  test("square_bracket_is_closed_after_cursor", () => {
    const state = stateAt("ls ", 3);

    const result = typed(state, "[");

    expect(result).toEqual({ doc: "ls []", cursor: 4 });
  });

  test("curly_bracket_is_closed_after_cursor", () => {
    const state = stateAt("ls ", 3);

    const result = typed(state, "{");

    expect(result).toEqual({ doc: "ls {}", cursor: 4 });
  });

  test("bracket_before_double_quote_is_closed_for_interpolation", () => {
    const state = stateAt('$"a "', 4);

    const result = typed(state, "(");

    expect(result).toEqual({ doc: '$"a ()"', cursor: 5 });
  });

  test("bracket_before_single_quote_is_closed_for_interpolation", () => {
    const state = stateAt("$'a '", 4);

    const result = typed(state, "(");

    expect(result).toEqual({ doc: "$'a ()'", cursor: 5 });
  });

  test("bracket_in_comment_is_left_to_typing", () => {
    const state = stateAt("ls # see ", 9);

    const result = typed(state, "(");

    expect(result).toBeNull();
  });

  test("quote_is_left_to_quotes", () => {
    const state = stateAt("ls ", 3);

    const result = typed(state, '"');

    expect(result).toBeNull();
  });
});
