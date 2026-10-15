import { EditorState } from "@codemirror/state";
import { describe, expect, test } from "vite-plus/test";

import { highlights, tokensAt } from "./highlights";
import { lineEndings, splitLineEndings } from "./line-endings";

const stateOf = (text: string) => {
  const { doc, crlf } = splitLineEndings(text);
  return EditorState.create({ doc, extensions: [lineEndings(crlf), highlights] });
};

const typesIn = (state: EditorState) => {
  const tokens = state.field(highlights);
  return tokens.map(({ from, to, type }) => {
    const text = state.sliceDoc(from, to);
    return [text, type];
  });
};

describe("highlights", () => {
  test("each_token_takes_its_type", () => {
    const state = stateOf("ls -la # c");

    const types = typesIn(state);

    expect(types).toEqual([
      ["ls", "function"],
      ["-la", "flag"],
      ["# c", "comment"],
    ]);
  });

  test("tokens_after_crlf_are_where_they_are_in_doc", () => {
    const state = stateOf("ls\r\n'🧶' | $x");

    const types = typesIn(state);

    expect(types).toEqual([
      ["ls", "function"],
      ["'🧶'", "string"],
      ["|", "operator"],
      ["$x", "variable"],
    ]);
  });

  test("source_with_lone_surrogate_takes_no_tokens", () => {
    const state = stateOf("echo '\ud800'");

    const types = typesIn(state);

    expect(types).toEqual([]);
  });

  test("tokens_follow_edits", () => {
    const state = stateOf("ls");

    const edited = state.update({ changes: { from: 0, insert: "let x = 1\n" } }).state;
    const types = typesIn(edited);

    expect(types).toEqual([
      ["let", "keyword"],
      ["x", "variable"],
      ["=", "operator"],
      ["1", "number"],
      ["ls", "function"],
    ]);
  });

  test("tokens_at_position_are_tokens_touching_it", () => {
    const state = stateOf("ls -la");

    const tokens = tokensAt(state, 2);

    expect(tokens).toEqual([{ from: 0, to: 2, type: "function" }]);
  });

  test("tokens_at_start_of_token_include_it", () => {
    const state = stateOf("ls -la");

    const tokens = tokensAt(state, 3);

    expect(tokens).toEqual([{ from: 3, to: 6, type: "flag" }]);
  });
});
