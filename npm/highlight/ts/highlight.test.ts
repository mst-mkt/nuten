import { describe, expect, test } from "vite-plus/test";

import { highlight } from "./default";

describe("source", () => {
  test("source_other_than_string_is_rejected", () => {
    const source = 1;

    // @ts-expect-error
    const highlighted = () => highlight(source);

    expect(highlighted).toThrow(new TypeError("`source` takes a string"));
  });

  test("source_with_lone_surrogate_is_rejected", () => {
    const source = "echo '\ud800'";

    const highlighted = () => highlight(source);

    expect(highlighted).toThrow(new TypeError("`source` has a lone surrogate"));
  });
});

describe("highlights", () => {
  test("highlights_are_plain_objects_of_range_and_type_in_order", () => {
    const source = "ls -la # c";

    const highlights = highlight(source);

    expect(highlights).toStrictEqual([
      { start: 0, end: 2, type: "function" },
      { start: 3, end: 6, type: "flag" },
      { start: 7, end: 10, type: "comment" },
    ]);
  });
});
