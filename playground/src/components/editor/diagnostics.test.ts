import { EditorState } from "@codemirror/state";
import { describe, expect, test } from "vite-plus/test";

import { formatResultOf, syntaxErrorsOf } from "../../lib/format";
import { keepCarriageReturns } from "./carriage-returns";
import { diagnosticsOf, errorLineStartsOf } from "./diagnostics";

const docOf = (source: string) => {
  return EditorState.create({ doc: source, extensions: keepCarriageReturns }).doc;
};

const rangeOf = (source: string, line: number, column: number) => {
  const [diagnostic] = diagnosticsOf(docOf(source), source, [{ line, column, message: "message" }]);
  return diagnostic && { from: diagnostic.from, to: diagnostic.to };
};

describe("diagnostics", () => {
  test("diagnostic_marks_point_of_line_and_column", () => {
    expect(rangeOf("ls\nfoo && bar", 2, 5)).toEqual({ from: 7, to: 7 });
  });

  test("diagnostic_counts_code_point_outside_basic_plane_as_two_code_units", () => {
    expect(rangeOf("'🧶' && x", 1, 5)).toEqual({ from: 5, to: 5 });
  });

  test("diagnostic_counts_carriage_return_into_line_before_line_feed", () => {
    expect(rangeOf("ls\r\nfoo && bar", 2, 5)).toEqual({ from: 8, to: 8 });
  });

  test("diagnostic_past_end_of_line_marks_end_of_line", () => {
    expect(rangeOf("ls |\nsort", 1, 9)).toEqual({ from: 4, to: 4 });
  });

  test("diagnostic_is_error_with_message_of_parse_error", () => {
    const [diagnostic] = diagnosticsOf(docOf("x"), "x", [{ line: 1, column: 1, message: "oops" }]);

    expect(diagnostic).toMatchObject({ severity: "error", message: "oops" });
  });

  test("diagnostic_of_syntax_error_marks_its_token", () => {
    const source = "'🧶'\r\nfoo && bar";
    const doc = docOf(source);
    const [diagnostic] = diagnosticsOf(doc, source, syntaxErrorsOf(formatResultOf(source, {})));

    expect(diagnostic && doc.sliceString(diagnostic.from, diagnostic.from + 2)).toBe("&&");
  });

  test("diagnostic_of_syntax_error_after_lone_carriage_return_marks_its_token", () => {
    const source = "ls\rfoo && bar";
    const doc = docOf(source);
    const [diagnostic] = diagnosticsOf(doc, source, syntaxErrorsOf(formatResultOf(source, {})));

    expect(diagnostic && doc.sliceString(diagnostic.from, diagnostic.from + 2)).toBe("&&");
  });

  test("diagnostics_of_errors_for_other_source_are_empty", () => {
    const errors = syntaxErrorsOf(formatResultOf("ls\nfoo && bar", {}));

    expect(diagnosticsOf(docOf("ls"), "ls\nfoo && bar", errors)).toEqual([]);
  });
});

describe("error lines", () => {
  test("error_line_starts_are_start_of_each_line_with_errors_once", () => {
    const source = "ls\nfoo && bar || baz\nsort\nx && y";
    const errors = syntaxErrorsOf(formatResultOf(source, {}));

    expect(errorLineStartsOf(docOf(source), source, errors)).toEqual([3, 26]);
  });

  test("error_line_starts_of_errors_for_other_source_are_empty", () => {
    const errors = syntaxErrorsOf(formatResultOf("ls\nfoo && bar", {}));

    expect(errorLineStartsOf(docOf("ls"), "ls\nfoo && bar", errors)).toEqual([]);
  });
});
