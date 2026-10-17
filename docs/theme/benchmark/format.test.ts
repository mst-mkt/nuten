import { describe, expect, test } from "vite-plus/test";

import { digits, milliseconds } from "./format";

describe("digits", () => {
  test("digits_are_grouped_by_three_with_commas", () => {
    const value = 1234567.891;

    const grouped = digits(value, 2);

    expect(grouped).toBe("1,234,567.89");
  });
});

describe("milliseconds", () => {
  test("milliseconds_keep_one_decimal_below_ten", () => {
    const nanoseconds = [9_440_000, 58_132_006];

    const formatted = nanoseconds.map(milliseconds);

    expect(formatted).toStrictEqual(["9.4 ms", "58 ms"]);
  });
});
