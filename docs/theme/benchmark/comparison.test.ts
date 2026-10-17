import { describe, expect, test } from "vite-plus/test";

import { astChanges, median, percentile, type Excluded } from "./comparison";

const comparisonWith = (excluded: Excluded[]) => ({
  measured_at: "2026-10-18T00:00:00Z",
  runs: 1,
  versions: { nuten: null, nufmt: null, nushell: null, corpus: {} },
  environment: { os: "linux", cpu: "cpu", cpus: 1 },
  files: 0,
  excluded,
  timing: { files: [] },
  directories: [],
  sizes: { nuten: 0, nufmt: 0 },
});

describe("astChanges", () => {
  test("files_that_change_the_ast_are_counted_per_formatter", () => {
    const comparison = comparisonWith([
      { formatter: "nufmt", reason: "changes the AST", files: ["a.nu", "b.nu"] },
      { formatter: "nufmt", reason: "times out", files: ["c.nu"] },
      { formatter: "nuten", reason: "rejects a file nu rejects", files: ["d.nu"] },
      { formatter: "nuten", reason: "changes the AST", files: ["e.nu"] },
    ]);

    const changes = astChanges(comparison);

    expect(changes).toStrictEqual({ nuten: 1, nufmt: 2 });
  });

  test("files_nuten_fails_on_are_rejected_since_their_ast_is_unchecked", () => {
    const comparison = comparisonWith([{ formatter: "nuten", reason: "fails", files: ["a.nu"] }]);

    const changes = () => astChanges(comparison);

    expect(changes).toThrow("nuten fails on some files");
  });
});

describe("median", () => {
  test("median_of_an_even_count_is_the_mean_of_the_middle_two", () => {
    const values = [4, 1, 3, 2];

    const middle = median(values);

    expect(middle).toBe(2.5);
  });

  test("median_of_no_values_is_rejected", () => {
    const middle = () => median([]);

    expect(middle).toThrow("no values to take the median of");
  });
});

describe("percentile", () => {
  test("percentile_takes_the_nearest_rank", () => {
    const values = [5, 1, 4, 2, 3, 6, 7, 8, 9, 10];

    const ranks = [0, 10, 50, 90].map((percent) => percentile(values, percent));

    expect(ranks).toStrictEqual([1, 1, 5, 9]);
  });

  test("percentile_of_no_values_is_rejected", () => {
    const rank = () => percentile([], 50);

    expect(rank).toThrow("no values to take a percentile of");
  });
});
