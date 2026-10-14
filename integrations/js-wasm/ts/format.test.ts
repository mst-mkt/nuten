import { describe, expect, test } from "vite-plus/test";

import { FormatError, defaultOptions, format } from "./node";

const block = "def f [] { [1 2 3] }";

const formatErrorOf = (run: () => unknown): FormatError => {
  try {
    run();
  } catch (error) {
    if (error instanceof FormatError) {
      return error;
    }
    throw error;
  }
  throw new Error("nothing was thrown");
};

describe("source", () => {
  test("source_other_than_string_is_rejected", () => {
    // @ts-expect-error
    expect(() => format(1)).toThrow(new TypeError("`source` takes a string"));
  });

  test("source_with_lone_surrogate_is_rejected", () => {
    expect(() => format("echo '\ud800'")).toThrow(new TypeError("`source` has a lone surrogate"));
  });
});

describe("options", () => {
  test("options_left_out_or_undefined_take_defaults_of_nuten", () => {
    const list = "[\n1\n2\n]";

    expect(format(block)).toBe("def f [] {\n    [1, 2, 3]\n}\n");
    expect(format(list)).toBe("[\n    1\n    2\n]\n");
    expect(format(block, { lineWidth: undefined })).toBe(format(block));
  });

  test("line_width_sets_where_lines_break", () => {
    expect(format(block, { lineWidth: 10 })).toBe(
      "def f [] {\n    [\n        1\n        2\n        3\n    ]\n}\n",
    );
  });

  test("indent_style_sets_what_indents", () => {
    expect(format(block, { indentStyle: "tab" })).toBe("def f [] {\n\t[1, 2, 3]\n}\n");
  });

  test("indent_width_sets_columns_per_indent", () => {
    expect(format(block, { indentWidth: 2 })).toBe("def f [] {\n  [1, 2, 3]\n}\n");
  });

  test("line_ending_sets_what_ends_each_line", () => {
    expect(format(block, { lineEnding: "crlf" })).toBe("def f [] {\r\n    [1, 2, 3]\r\n}\r\n");
  });

  test("commas_sets_where_commas_go", () => {
    expect(format(block, { commas: "none" })).toBe("def f [] {\n    [1 2 3]\n}\n");
  });

  test("options_other_than_object_are_rejected", () => {
    // @ts-expect-error
    expect(() => format(block, null)).toThrow(new TypeError("`options` takes an object"));
  });

  test("option_set_to_null_is_rejected", () => {
    const options = { lineWidth: null };

    // @ts-expect-error
    expect(() => format(block, options)).toThrow(
      new TypeError("`lineWidth` takes an integer from 1 to 4294967295"),
    );
  });

  test("unknown_option_is_rejected", () => {
    const options = { linewidth: 10 };

    // @ts-expect-error
    expect(() => format(block, options)).toThrow(new TypeError("unknown option `linewidth`"));
  });

  test("integer_option_rejects_number_out_of_range", () => {
    expect(() => format(block, { lineWidth: 0 })).toThrow(
      new TypeError("`lineWidth` takes an integer from 1 to 4294967295"),
    );
  });

  test("choice_option_rejects_unlisted_name", () => {
    const options = { indentStyle: "Tab" };

    // @ts-expect-error
    expect(() => format(block, options)).toThrow(
      new TypeError("`indentStyle` takes one of `space`, `tab`"),
    );
  });

  test("integer_option_rejects_string", () => {
    const options = { lineWidth: "10" };

    // @ts-expect-error
    expect(() => format(block, options)).toThrow(
      new TypeError("`lineWidth` takes an integer from 1 to 4294967295"),
    );
  });

  test("choice_option_rejects_number", () => {
    const options = { indentStyle: 1 };

    // @ts-expect-error
    expect(() => format(block, options)).toThrow(
      new TypeError("`indentStyle` takes one of `space`, `tab`"),
    );
  });
});

describe("default options", () => {
  test("default_options_are_defaults_of_nuten", () => {
    expect(defaultOptions).toEqual({
      lineWidth: 100,
      indentStyle: "space",
      indentWidth: 4,
      lineEnding: "auto",
      commas: "inline",
    });
  });

  test("default_options_format_as_options_left_out", () => {
    const source = `def f [] {\r\n[1 2]\r\n[\n1\n2\n]\r\n[${"item ".repeat(30)}]\r\n}`;

    expect(format(source, defaultOptions)).toBe(format(source));
  });

  test("default_options_are_frozen", () => {
    expect(Object.isFrozen(defaultOptions)).toBe(true);
  });
});

describe("syntax errors", () => {
  test("syntax_errors_are_thrown_as_format_error_with_each_place", () => {
    const error = formatErrorOf(() => format("ls\nfoo && bar || baz\n"));

    expect(error).toMatchObject({
      name: "FormatError",
      message:
        "2:5: `&&` is not an operator; use `and` (`;` for the shell meaning)\n" +
        "2:12: `||` is not an operator; use `or` (`try` for the shell meaning)",
    });
    expect(error.errors).toStrictEqual([
      {
        line: 2,
        column: 5,
        message: "`&&` is not an operator; use `and` (`;` for the shell meaning)",
      },
      {
        line: 2,
        column: 12,
        message: "`||` is not an operator; use `or` (`try` for the shell meaning)",
      },
    ]);
  });

  test("syntax_error_column_counts_code_points", () => {
    const error = formatErrorOf(() => format("'🐢' && x"));

    expect(error.errors[0]).toMatchObject({ line: 1, column: 5 });
  });
});
