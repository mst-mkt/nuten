import { EditorState, type TransactionSpec } from "@codemirror/state";
import { describe, expect, test } from "vite-plus/test";

import { highlights } from "./highlights";
import { pairDeleted, quoteTyped } from "./quotes";

const stateAt = (doc: string, cursor: number) => {
  const selection = { anchor: cursor };
  return EditorState.create({ doc, selection, extensions: highlights });
};

const applied = (state: EditorState, spec: TransactionSpec | null) => {
  if (spec === null) return null;

  const next = state.update(spec).state;
  const doc = next.doc.toString();
  const cursor = next.selection.main.head;
  return { doc, cursor };
};

const typed = (state: EditorState, quote: string) => {
  const cursor = state.selection.main.head;
  const spec = quoteTyped(state, cursor, quote);
  return applied(state, spec);
};

const backspaced = (state: EditorState) => {
  const cursor = state.selection.main.head;
  const spec = pairDeleted(state, cursor);
  return applied(state, spec);
};

describe("typing quote", () => {
  test("quote_outside_string_is_closed_after_cursor", () => {
    const state = stateAt("echo ", 5);

    const result = typed(state, '"');

    expect(result).toEqual({ doc: 'echo ""', cursor: 6 });
  });

  test("quote_before_line_break_is_closed_after_cursor", () => {
    const state = stateAt("echo \nls", 5);

    const result = typed(state, '"');

    expect(result).toEqual({ doc: 'echo ""\nls', cursor: 6 });
  });

  test("quote_before_closing_bracket_is_closed_after_cursor", () => {
    const state = stateAt("[]", 1);

    const result = typed(state, '"');

    expect(result).toEqual({ doc: '[""]', cursor: 2 });
  });

  test("quote_after_closed_string_at_end_is_closed_after_cursor", () => {
    const state = stateAt('echo "a" ', 9);

    const result = typed(state, '"');

    expect(result).toEqual({ doc: 'echo "a" ""', cursor: 10 });
  });

  test("quote_in_interpolated_expression_is_closed_after_cursor", () => {
    const state = stateAt('$"a ()"', 5);

    const result = typed(state, "'");

    expect(result).toEqual({ doc: `$"a ('')"`, cursor: 6 });
  });

  test("quote_before_closing_quote_steps_over_it", () => {
    const state = stateAt('echo "a"', 7);

    const result = typed(state, '"');

    expect(result).toEqual({ doc: 'echo "a"', cursor: 8 });
  });

  test("quote_before_closing_quote_of_interpolation_steps_over_it", () => {
    const state = stateAt('$"a (1)"', 7);

    const result = typed(state, '"');

    expect(result).toEqual({ doc: '$"a (1)"', cursor: 8 });
  });

  test("quote_before_closing_quote_of_other_kind_is_left_to_typing", () => {
    const state = stateAt('echo "a"', 7);

    const result = typed(state, "'");

    expect(result).toBeNull();
  });

  test("quote_before_same_quote_inside_string_is_left_to_typing", () => {
    const state = stateAt("echo 'a\"b'", 7);

    const result = typed(state, '"');

    expect(result).toBeNull();
  });

  test("quote_after_backslash_in_string_at_end_is_left_to_typing", () => {
    const state = stateAt('echo "say \\"', 11);

    const result = typed(state, '"');

    expect(result).toBeNull();
  });

  test("quote_in_string_is_left_to_typing", () => {
    const state = stateAt('echo "a b"', 7);

    const result = typed(state, "'");

    expect(result).toBeNull();
  });

  test("quote_in_unclosed_string_is_left_to_typing", () => {
    const state = stateAt('echo "a ', 8);

    const result = typed(state, '"');

    expect(result).toBeNull();
  });

  test("quote_in_comment_is_left_to_typing", () => {
    const state = stateAt("# ", 2);

    const result = typed(state, '"');

    expect(result).toBeNull();
  });

  test("quote_opening_raw_string_is_left_to_typing", () => {
    const state = stateAt("echo r#", 7);

    const result = typed(state, "'");

    expect(result).toBeNull();
  });

  test("quote_after_word_is_left_to_typing", () => {
    const state = stateAt("echo don", 8);

    const result = typed(state, "'");

    expect(result).toBeNull();
  });

  test("quote_after_word_of_other_script_is_left_to_typing", () => {
    const state = stateAt("echo é", 6);

    const result = typed(state, "'");

    expect(result).toBeNull();
  });

  test("quote_before_word_is_left_to_typing", () => {
    const state = stateAt("echo foo", 5);

    const result = typed(state, '"');

    expect(result).toBeNull();
  });
});

describe("deleting quote", () => {
  test("backspace_in_empty_string_deletes_both_quotes", () => {
    const state = stateAt('echo ""', 6);

    const result = backspaced(state);

    expect(result).toEqual({ doc: "echo ", cursor: 5 });
  });

  test("backspace_in_empty_backtick_string_deletes_both_backticks", () => {
    const state = stateAt("echo ``", 6);

    const result = backspaced(state);

    expect(result).toEqual({ doc: "echo ", cursor: 5 });
  });

  test("backspace_after_opening_quote_of_string_with_text_is_left_to_typing", () => {
    const state = stateAt('echo "a"', 6);

    const result = backspaced(state);

    expect(result).toBeNull();
  });

  test("backspace_between_quotes_of_different_kinds_is_left_to_typing", () => {
    const state = stateAt("echo \"'", 6);

    const result = backspaced(state);

    expect(result).toBeNull();
  });

  test("backspace_between_quotes_inside_string_is_left_to_typing", () => {
    const state = stateAt("echo 'a\"\"'", 8);

    const result = backspaced(state);

    expect(result).toBeNull();
  });
});
