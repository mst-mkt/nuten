import {
  copyLineDown,
  history,
  insertNewlineAndIndent,
  moveLineDown,
  moveLineUp,
  redo,
  undo,
} from "@codemirror/commands";
import { EditorState, type StateCommand, type TransactionSpec } from "@codemirror/state";
import { describe, expect, test } from "vite-plus/test";

import { joinLineEndings, lineEndings, splitLineEndings, textOf } from "./line-endings";

const stateOf = (text: string, cursor = 0) => {
  const { doc, crlf } = splitLineEndings(text);
  const extensions = [history(), lineEndings(crlf)];
  return EditorState.create({ doc, selection: { anchor: cursor }, extensions });
};

const ran = (command: StateCommand, state: EditorState) => {
  const states = [state];
  command({ state, dispatch: (tr) => states.push(tr.state) });
  return states.at(-1) ?? state;
};

const updated = (state: EditorState, spec: TransactionSpec) => {
  return state.update(spec).state;
};

describe("split and join", () => {
  test("split_takes_carriage_return_of_each_crlf_out_and_keeps_its_line_break", () => {
    const text = "a\r\nb\nc\r\n";

    const split = splitLineEndings(text);

    expect(split).toEqual({ doc: "a\nb\nc\n", crlf: [1, 5] });
  });

  test("split_keeps_lone_carriage_return_as_character", () => {
    const text = "a\rb\r\r\nc";

    const split = splitLineEndings(text);

    expect(split).toEqual({ doc: "a\rb\r\nc", crlf: [4] });
  });

  test("join_puts_carriage_return_before_each_crlf_line_break", () => {
    const doc = "a\nb\nc\n";

    const text = joinLineEndings(doc, [1, 5]);

    expect(text).toBe("a\r\nb\nc\r\n");
  });

  test("text_of_editor_is_text_given_to_it", () => {
    const text = "a\r\nb\rc\nd\r\n";

    const state = stateOf(text);

    expect(textOf(state)).toBe(text);
    expect(state.doc.lines).toBe(4);
  });
});

describe("typed line breaks", () => {
  test("line_break_typed_on_crlf_line_takes_crlf", () => {
    const state = stateOf("ab\r\ncd", 1);

    const next = ran(insertNewlineAndIndent, state);

    expect(textOf(next)).toBe("a\r\nb\r\ncd");
  });

  test("line_break_typed_at_end_of_crlf_line_keeps_its_crlf", () => {
    const state = stateOf("ab\r\ncd", 2);

    const next = ran(insertNewlineAndIndent, state);

    expect(textOf(next)).toBe("ab\r\n\r\ncd");
    expect(next.selection.main.head).toBe(3);
  });

  test("line_break_typed_on_lf_line_stays_lf", () => {
    const state = stateOf("ab\ncd\r\nef", 1);

    const next = ran(insertNewlineAndIndent, state);

    expect(textOf(next)).toBe("a\nb\ncd\r\nef");
  });

  test("line_break_typed_on_last_line_takes_line_ending_of_line_above", () => {
    const state = stateOf("a\r\nb", 3);

    const next = ran(insertNewlineAndIndent, state);

    expect(textOf(next)).toBe("a\r\nb\r\n");
  });

  test("line_break_typed_in_text_of_one_line_is_lf", () => {
    const state = stateOf("ab", 1);

    const next = ran(insertNewlineAndIndent, state);

    expect(textOf(next)).toBe("a\nb");
  });

  test("text_typed_at_end_of_crlf_line_goes_before_its_line_ending", () => {
    const state = stateOf("ab\r\ncd", 2);

    const next = updated(state, {
      changes: { from: 2, insert: "x" },
      selection: { anchor: 3 },
      userEvent: "input.type",
    });

    expect(textOf(next)).toBe("abx\r\ncd");
  });

  test("text_typed_into_empty_text_is_kept", () => {
    const state = stateOf("");

    const next = updated(state, { changes: { from: 0, insert: "x" }, userEvent: "input.type" });

    expect(textOf(next)).toBe("x");
  });
});

describe("deleted line breaks", () => {
  test("deleting_crlf_line_break_joins_lines", () => {
    const state = stateOf("ab\r\ncd", 3);

    const next = updated(state, { changes: { from: 2, to: 3 }, userEvent: "delete.backward" });

    expect(textOf(next)).toBe("abcd");
  });

  test("deleting_text_before_crlf_keeps_crlf", () => {
    const state = stateOf("ab\r\ncd", 0);

    const next = updated(state, { changes: { from: 0, to: 2 }, userEvent: "delete.forward" });

    expect(textOf(next)).toBe("\r\ncd");
  });
});

describe("history", () => {
  test("undoing_joined_crlf_line_brings_back_crlf", () => {
    const joined = updated(stateOf("ab\r\ncd", 3), {
      changes: { from: 2, to: 3 },
      userEvent: "delete.backward",
    });

    const undone = ran(undo, joined);

    expect(textOf(undone)).toBe("ab\r\ncd");
  });

  test("undoing_deletions_merged_into_one_event_brings_back_crlf_in_place", () => {
    const deleteForward = { changes: { from: 1, to: 2 }, userEvent: "delete.forward" };
    const once = updated(stateOf("ax\r\nb", 1), deleteForward);
    const twice = updated(once, deleteForward);

    const undone = ran(undo, twice);

    expect(textOf(undone)).toBe("ax\r\nb");
  });

  test("redoing_typed_line_break_brings_back_crlf", () => {
    const typed = ran(insertNewlineAndIndent, stateOf("ab\r\ncd", 1));
    const undone = ran(undo, typed);

    const redone = ran(redo, undone);

    expect(textOf(redone)).toBe("a\r\nb\r\ncd");
  });

  test("redoing_pasted_crlf_brings_back_crlf", () => {
    const pasted = updated(stateOf("ab"), {
      changes: { from: 0, insert: "x\r\ny" },
      userEvent: "input.paste",
    });
    const undone = ran(undo, pasted);

    const redone = ran(redo, undone);

    expect(textOf(redone)).toBe("x\r\nyab");
  });
});

describe("moved lines", () => {
  test("line_moved_down_takes_line_ending_of_place_it_moves_to", () => {
    const state = stateOf("ab\r\ncd", 0);

    const next = ran(moveLineDown, state);

    expect(textOf(next)).toBe("cd\r\nab");
  });

  test("line_moved_up_takes_line_ending_of_place_it_moves_to", () => {
    const state = stateOf("ab\r\ncd", 3);

    const next = ran(moveLineUp, state);

    expect(textOf(next)).toBe("cd\r\nab");
  });

  test("line_copied_down_takes_crlf_of_crlf_line", () => {
    const state = stateOf("ab\r\ncd", 0);

    const next = ran(copyLineDown, state);

    expect(textOf(next)).toBe("ab\r\nab\r\ncd");
  });
});

describe("inserted text", () => {
  test("pasted_crlf_stays_crlf_without_carriage_return_in_editor", () => {
    const state = stateOf("ab", 0);

    const next = updated(state, {
      changes: { from: 0, insert: "x\r\ny" },
      selection: { anchor: 4 },
      userEvent: "input.paste",
    });

    expect(textOf(next)).toBe("x\r\nyab");
    expect(next.doc.toString()).toBe("x\nyab");
    expect(next.selection.main.head).toBe(3);
  });

  test("pasted_lf_stays_lf_on_crlf_line", () => {
    const state = stateOf("ab\r\ncd", 0);

    const next = updated(state, { changes: { from: 0, insert: "x\ny" }, userEvent: "input.paste" });

    expect(textOf(next)).toBe("x\nyab\r\ncd");
  });

  test("text_set_from_outside_keeps_its_line_endings", () => {
    const state = stateOf("ab\ncd");

    const next = updated(state, { changes: { from: 0, to: 5, insert: "p\r\nq\nr" } });

    expect(textOf(next)).toBe("p\r\nq\nr");
  });
});
