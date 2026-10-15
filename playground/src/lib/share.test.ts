import { defaultOptions } from "@nuten/format";
import { afterEach, beforeEach, describe, expect, test, vi } from "vite-plus/test";

import { createPlaygroundStore } from "./playground";
import { keepInUrl, sharedIn, sharedUrlOf } from "./share";

const page = "https://nuten.keito.dev/playground";
const sample = "ls | sort\n";

const roundTrip = (source: string) => {
  const url = sharedUrlOf(page, { source, options: defaultOptions }, sample);
  return sharedIn(url, sample).source;
};

const optionsIn = (query: string) => sharedIn(`${page}?${query}`, sample).options;

describe("share", () => {
  test("sample_with_default_options_is_shared_as_bare_page", () => {
    expect(sharedUrlOf(page, { source: sample, options: defaultOptions }, sample)).toBe(page);
  });

  test("bare_page_opens_sample_with_default_options", () => {
    expect(sharedIn(page, sample)).toEqual({ source: sample, options: defaultOptions });
  });

  test("code_comes_back_as_written", () => {
    expect(roundTrip("let x = 1\r\nls | sort\n")).toBe("let x = 1\r\nls | sort\n");
    expect(roundTrip("print 日本語 🍣\r")).toBe("print 日本語 🍣\r");
  });

  test("empty_code_comes_back_empty", () => {
    expect(sharedUrlOf(page, { source: "", options: defaultOptions }, sample)).toBe(
      `${page}#code/`,
    );
    expect(roundTrip("")).toBe("");
  });

  test("code_goes_in_hash_with_only_url_safe_characters", () => {
    const url = new URL(sharedUrlOf(page, { source: "ls", options: defaultOptions }, sample));

    expect(url.search).toBe("");
    expect(url.hash).toMatch(/^#code\/[\w+$-]+$/);
  });

  test("only_changed_options_go_in_query", () => {
    const options = { ...defaultOptions, lineWidth: 80, indentStyle: "tab" } as const;

    const url = new URL(sharedUrlOf(page, { source: "ls", options }, sample));

    expect(url.search).toBe("?line_width=80&indent_style=tab");
    expect(sharedIn(url.href, sample)).toEqual({ source: "ls", options });
  });

  test("every_option_goes_by_its_name_in_config", () => {
    const options = {
      lineWidth: 1,
      indentStyle: "tab",
      indentWidth: 8,
      lineEnding: "crlf",
      commas: "trailing",
    } as const;
    const query = "line_width=1&indent_style=tab&indent_width=8&line_ending=crlf&commas=trailing";

    const url = new URL(sharedUrlOf(page, { source: "ls", options }, sample));

    expect(url.search).toBe(`?${query}`);
    expect(optionsIn(query)).toEqual(options);
  });

  test("sample_with_changed_options_keeps_its_code", () => {
    const options = { ...defaultOptions, commas: "always" } as const;

    const url = new URL(sharedUrlOf(page, { source: sample, options }, sample));

    expect(url.hash).toMatch(/^#code\/./);
    expect(sharedIn(url.href, "")).toEqual({ source: sample, options });
  });

  test("shared_url_replaces_query_and_hash_of_current_page", () => {
    const current = `${page}?line_width=80&utm=1#code/abc`;

    expect(sharedUrlOf(current, { source: sample, options: defaultOptions }, sample)).toBe(page);
  });

  test("widths_up_to_their_limits_are_taken", () => {
    expect(optionsIn("line_width=4294967295&indent_width=255")).toEqual({
      ...defaultOptions,
      lineWidth: 4294967295,
      indentWidth: 255,
    });
  });

  test("invalid_option_falls_back_to_default", () => {
    const widths = ["0", "1.5", "080", "1e3", "0x50", "+80", "80 ", "80px", "99999999999999999"];
    const choices = ["tabs", "CRLF", "", "constructor", "toString", "__proto__"];

    for (const width of widths) {
      expect(optionsIn(`line_width=${encodeURIComponent(width)}`)).toEqual(defaultOptions);
    }
    expect(optionsIn("line_width=4294967296&indent_width=256")).toEqual(defaultOptions);
    for (const choice of choices) {
      const query = `indent_style=${choice}&line_ending=${choice}&commas=${choice}`;
      expect(optionsIn(query)).toEqual(defaultOptions);
    }
  });

  test("unknown_option_is_ignored", () => {
    expect(optionsIn("lineWidth=80&tab_width=2")).toEqual(defaultOptions);
  });

  test("broken_code_falls_back_to_sample", () => {
    expect(sharedIn(`${page}#code/abc`, sample).source).toBe(sample);
    expect(sharedIn(`${page}#code/zzzzzzzzzz`, sample).source).toBe(sample);
    expect(sharedIn(`${page}#code/%E0`, sample).source).toBe(sample);
    expect(sharedIn(`${page}#other`, sample).source).toBe(sample);
  });

  test("cut_link_falls_back_to_sample", () => {
    const source = "def greet [name: string] {\n  $'hello ($name)'\n}\n\ngreet nushell\n";
    const url = sharedUrlOf(page, { source, options: defaultOptions }, sample);
    const code = new URL(url).hash.slice("#code/".length);

    for (let end = 1; end < code.length; end++) {
      expect(sharedIn(`${page}#code/${code.slice(0, end)}`, sample).source).toBe(sample);
    }
  });

  test("percent_encoded_code_comes_back", () => {
    const source = "[1 2] | math sum";
    const url = new URL(sharedUrlOf(page, { source, options: defaultOptions }, sample));
    const code = url.hash.slice("#code/".length);
    const encoded = code.replaceAll("$", "%24").replaceAll("+", "%2B");

    expect(code).toMatch(/[$+]/);
    expect(sharedIn(`${page}#code/${encoded}`, sample).source).toBe(source);
  });

  test("plus_turned_into_space_comes_back", () => {
    const source = "[1 2] | each {|x| $x + 1 } | math sum";
    const url = new URL(sharedUrlOf(page, { source, options: defaultOptions }, sample));
    const code = url.hash.slice("#code/".length);

    expect(code.split("+").length).toBeGreaterThan(2);
    expect(sharedIn(`${page}#code/${code.replaceAll("+", " ")}`, sample).source).toBe(source);
  });
});

describe("url sync", () => {
  const listeners = new Map<string, () => void>();
  const reload = vi.fn();
  const replaceState = vi.fn((_state: unknown, _unused: string, url: string) => {
    location.href = url;
  });

  beforeEach(() => {
    vi.useFakeTimers();
    vi.stubGlobal("location", { href: page, reload });
    vi.stubGlobal("history", { state: { key: 1 }, replaceState });
    vi.stubGlobal("addEventListener", (type: string, listener: () => void) => {
      listeners.set(type, listener);
    });
    vi.stubGlobal("removeEventListener", (type: string, listener: () => void) => {
      if (listeners.get(type) === listener) listeners.delete(type);
    });
  });

  afterEach(() => {
    listeners.clear();
    reload.mockClear();
    replaceState.mockClear();
    vi.unstubAllGlobals();
    vi.useRealTimers();
  });

  test("url_follows_last_change_after_pause", () => {
    const store = createPlaygroundStore(sample);
    keepInUrl(store, sample);

    store.setSource("ls");
    vi.advanceTimersByTime(300);
    store.setSource("sort");
    vi.advanceTimersByTime(499);

    expect(replaceState).not.toHaveBeenCalled();

    vi.advanceTimersByTime(1);

    expect(replaceState).toHaveBeenCalledOnce();
    expect(replaceState).toHaveBeenCalledWith({ key: 1 }, "", location.href);
    expect(sharedIn(location.href, sample).source).toBe("sort");
  });

  test("url_is_left_alone_when_it_already_matches", () => {
    const store = createPlaygroundStore(sample);
    keepInUrl(store, sample);

    store.setSource(sample);
    vi.advanceTimersByTime(500);

    expect(replaceState).not.toHaveBeenCalled();
  });

  test("new_hash_reloads_page", () => {
    keepInUrl(createPlaygroundStore(sample), sample);

    listeners.get("hashchange")?.();

    expect(reload).toHaveBeenCalledOnce();
  });

  test("nothing_follows_after_cleanup", () => {
    const store = createPlaygroundStore(sample);
    const cleanup = keepInUrl(store, sample);

    store.setSource("ls");
    cleanup();
    store.setSource("sort");
    vi.advanceTimersByTime(500);

    expect(replaceState).not.toHaveBeenCalled();
    expect(listeners.has("hashchange")).toBe(false);
  });
});
