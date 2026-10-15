import { EditorState } from "@codemirror/state";
import { describe, expect, test } from "vite-plus/test";

import { comments } from "./comments";

describe("comments", () => {
  test("line_comments_start_with_hash", () => {
    const state = EditorState.create({ doc: "ls", extensions: comments });

    const tokens = state.languageDataAt("commentTokens", 0);

    expect(tokens).toEqual([{ line: "#" }]);
  });
});
