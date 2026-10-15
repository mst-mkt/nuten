import { EditorState } from "@codemirror/state";
import { describe, expect, test } from "vite-plus/test";

import { keepCarriageReturns } from "./carriage-returns";

describe("carriage returns", () => {
  test("editor_keeps_carriage_returns_of_source", () => {
    const source = "ls\r\nsort\rfoo\n";
    const state = EditorState.create({ doc: source, extensions: keepCarriageReturns });

    expect(state.doc.toString()).toBe(source);
    expect(state.doc.lines).toBe(3);
  });
});
