import { EditorState } from "@codemirror/state";
import { describe, expect, test } from "vite-plus/test";

import { formatResultOf, syntaxErrorsOf } from "../../lib/format";
import { diagnosticsOf, errorLineStartsOf } from "./diagnostics";
import { lineEndings, splitLineEndings } from "./line-endings";

const stateOf = (text: string) => {
  const { doc, crlf } = splitLineEndings(text);
  return EditorState.create({ doc, extensions: lineEndings(crlf) });
};

const errorsIn = (source: string) => {
  return { source, errors: syntaxErrorsOf(formatResultOf(source, {})) };
};

const rangeOf = (source: string, line: number, column: number) => {
  const errors = [{ line, column, message: "message" }];
  const [diagnostic] = diagnosticsOf(stateOf(source), { source, errors });
  return diagnostic && { from: diagnostic.from, to: diagnostic.to };
};

describe("diagnostics", () => {
  test("diagnostic_marks_point_of_line_and_column", () => {
    const source = "ls\nfoo && bar";

    const range = rangeOf(source, 2, 5);

    expect(range).toEqual({ from: 7, to: 7 });
  });

  test("diagnostic_counts_code_point_outside_basic_plane_as_two_code_units", () => {
    const source = "'🧶' && x";

    const range = rangeOf(source, 1, 5);

    expect(range).toEqual({ from: 5, to: 5 });
  });

  test("diagnostic_counts_crlf_as_one_line_break", () => {
    const source = "ls\r\nfoo && bar";

    const range = rangeOf(source, 2, 5);

    expect(range).toEqual({ from: 7, to: 7 });
  });

  test("diagnostic_past_end_of_line_marks_end_of_line", () => {
    const source = "ls |\nsort";

    const range = rangeOf(source, 1, 9);

    expect(range).toEqual({ from: 4, to: 4 });
  });

  test("diagnostic_is_error_with_message_of_parse_error", () => {
    const errors = [{ line: 1, column: 1, message: "oops" }];

    const [diagnostic] = diagnosticsOf(stateOf("x"), { source: "x", errors });

    expect(diagnostic).toMatchObject({ severity: "error", message: "oops" });
  });

  test("diagnostic_of_syntax_error_after_crlf_marks_its_token", () => {
    const source = "'🧶'\r\nfoo && bar";
    const state = stateOf(source);

    const [diagnostic] = diagnosticsOf(state, errorsIn(source));

    expect(diagnostic && state.sliceDoc(diagnostic.from, diagnostic.from + 2)).toBe("&&");
  });

  test("diagnostic_of_syntax_error_after_lone_carriage_return_marks_its_token", () => {
    const source = "ls\rfoo && bar";
    const state = stateOf(source);

    const [diagnostic] = diagnosticsOf(state, errorsIn(source));

    expect(diagnostic && state.sliceDoc(diagnostic.from, diagnostic.from + 2)).toBe("&&");
  });

  test("diagnostics_of_errors_for_other_source_are_empty", () => {
    const errors = errorsIn("ls\nfoo && bar");

    const diagnostics = diagnosticsOf(stateOf("ls"), errors);

    expect(diagnostics).toEqual([]);
  });
});

describe("error lines", () => {
  test("error_line_starts_are_start_of_each_line_with_diagnostic_once", () => {
    const { doc } = stateOf("ls\nfoo && bar || baz\nsort\nx && y");

    const starts = errorLineStartsOf(doc, [7, 14, 29]);

    expect(starts).toEqual([3, 26]);
  });
});
