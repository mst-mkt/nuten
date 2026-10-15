import { defaultOptions } from "@nuten/format";
import { describe, expect, test } from "vite-plus/test";

import { createPlaygroundStore } from "./playground";

describe("playground store", () => {
  test("store_starts_with_source_formatted_with_default_options", () => {
    const store = createPlaygroundStore("ls|sort");

    const snapshot = store.getSnapshot();

    expect(snapshot).toEqual({
      source: "ls|sort",
      options: defaultOptions,
      result: { kind: "output", output: "ls | sort\n" },
    });
  });

  test("new_source_is_formatted_with_current_options", () => {
    const store = createPlaygroundStore("ls");
    store.setOptions({ ...defaultOptions, commas: "none" });

    store.setSource("[1, 2]");

    expect(store.getSnapshot().result).toEqual({ kind: "output", output: "[1 2]\n" });
  });

  test("new_options_format_current_source_again", () => {
    const store = createPlaygroundStore("[1 2]");

    store.setOptions({ ...defaultOptions, commas: "always" });

    expect(store.getSnapshot().result).toEqual({ kind: "output", output: "[1, 2]\n" });
  });

  test("store_starts_with_given_options", () => {
    const options = { ...defaultOptions, commas: "always" } as const;

    const store = createPlaygroundStore("[1 2]", options);

    expect(store.getSnapshot()).toEqual({
      source: "[1 2]",
      options,
      result: { kind: "output", output: "[1, 2]\n" },
    });
  });

  test("each_change_notifies_listeners_until_they_unsubscribe", () => {
    const store = createPlaygroundStore("ls");
    const sources: string[] = [];
    const unsubscribe = store.subscribe(() => sources.push(store.getSnapshot().source));

    store.setSource("sort");
    unsubscribe();
    store.setSource("uniq");

    expect(sources).toEqual(["sort"]);
  });

  test("snapshot_stays_same_object_without_change", () => {
    const store = createPlaygroundStore("ls");

    const first = store.getSnapshot();
    const second = store.getSnapshot();

    expect(first).toBe(second);
  });
});
