import { EditorState } from "@codemirror/state";
import { describe, expect, test } from "vite-plus/test";

import { highlights } from "./highlights";
import { lineEndings, splitLineEndings } from "./line-endings";

const stateOf = (text: string) => {
  const { doc, crlf } = splitLineEndings(text);
  return EditorState.create({ doc, extensions: [lineEndings(crlf), highlights] });
};

const marked = (state: EditorState) => {
  const found: [string, string][] = [];
  state.field(highlights).between(0, state.doc.length, (from, to, { spec }) => {
    found.push([state.sliceDoc(from, to), spec.class]);
  });
  return found;
};

describe("highlights", () => {
  test("each_token_is_marked_with_class_of_its_type", () => {
    expect(marked(stateOf("ls -la # c"))).toEqual([
      ["ls", "cm-nu-function"],
      ["-la", "cm-nu-flag"],
      ["# c", "cm-nu-comment"],
    ]);
  });

  test("tokens_after_crlf_are_marked_where_they_are_in_doc", () => {
    expect(marked(stateOf("ls\r\n'🧶' | $x"))).toEqual([
      ["ls", "cm-nu-function"],
      ["'🧶'", "cm-nu-string"],
      ["|", "cm-nu-operator"],
      ["$x", "cm-nu-variable"],
    ]);
  });

  test("source_with_lone_surrogate_takes_no_marks", () => {
    expect(marked(stateOf("echo '\ud800'"))).toEqual([]);
  });

  test("marks_follow_edits", () => {
    const state = stateOf("ls");

    const edited = state.update({ changes: { from: 0, insert: "let x = 1\n" } }).state;

    expect(marked(edited)).toEqual([
      ["let", "cm-nu-keyword"],
      ["x", "cm-nu-variable"],
      ["=", "cm-nu-operator"],
      ["1", "cm-nu-number"],
      ["ls", "cm-nu-function"],
    ]);
  });
});
