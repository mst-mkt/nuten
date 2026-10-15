import { describe, expect, test } from "vite-plus/test";

import { formatResultOf, messagesOf, syntaxErrorsOf } from "./format";

describe("format result", () => {
  test("source_without_syntax_errors_gives_formatted_output", () => {
    expect(formatResultOf("ls|sort", {})).toEqual({ kind: "output", output: "ls | sort\n" });
  });

  test("source_with_syntax_errors_gives_each_error", () => {
    expect(formatResultOf("ls && echo || x", {})).toMatchObject({
      kind: "syntax-errors",
      errors: [
        { line: 1, column: 4 },
        { line: 1, column: 12 },
      ],
    });
  });

  test("option_out_of_range_gives_message_of_nuten", () => {
    expect(formatResultOf("ls", { lineWidth: 0 })).toEqual({
      kind: "invalid-input",
      message: "`lineWidth` takes an integer from 1 to 4294967295",
    });
  });

  test("source_with_lone_surrogate_gives_message_of_nuten", () => {
    expect(formatResultOf("echo '\ud800'", {})).toEqual({
      kind: "invalid-input",
      message: "`source` has a lone surrogate",
    });
  });

  test("unexpected_error_in_formatting_gives_failure_instead_of_throwing", () => {
    const options = {
      get lineWidth(): number {
        throw new Error("broken");
      },
    };

    expect(formatResultOf("ls", options)).toEqual({ kind: "failure", message: "Error: broken" });
  });
});

describe("messages of format result", () => {
  test("messages_of_syntax_errors_are_placed_at_each_error", () => {
    expect(messagesOf(formatResultOf("ls && echo || x", {}))).toEqual([
      {
        position: { line: 1, column: 4 },
        text: "`&&` is not an operator; use `and` (`;` for the shell meaning)",
      },
      {
        position: { line: 1, column: 12 },
        text: "`||` is not an operator; use `or` (`try` for the shell meaning)",
      },
    ]);
  });

  test("message_of_invalid_input_has_no_position", () => {
    expect(messagesOf(formatResultOf("ls", { lineWidth: 0 }))).toEqual([
      { text: "`lineWidth` takes an integer from 1 to 4294967295" },
    ]);
  });

  test("messages_of_output_are_empty", () => {
    expect(messagesOf(formatResultOf("ls", {}))).toEqual([]);
  });
});

describe("syntax errors of format result", () => {
  test("syntax_errors_of_result_with_syntax_errors_are_its_errors", () => {
    expect(syntaxErrorsOf(formatResultOf("ls && echo || x", {}))).toMatchObject([
      { line: 1, column: 4 },
      { line: 1, column: 12 },
    ]);
  });

  test("syntax_errors_of_result_without_syntax_errors_are_empty", () => {
    expect(syntaxErrorsOf(formatResultOf("ls", {}))).toEqual([]);
    expect(syntaxErrorsOf(formatResultOf("ls", { lineWidth: 0 }))).toEqual([]);
  });
});
