import { type Options, type RequiredOptions, defaultOptions, format } from "@nuten/wasm";
import { compressToEncodedURIComponent, decompressFromEncodedURIComponent } from "lz-string";

import type { Playground, PlaygroundStore } from "./playground";

export type Shared = Pick<Playground, "source" | "options">;

const codePrefix = "#code/";
const urlDelay = 500;

const optionNames = {
  lineWidth: "line_width",
  indentStyle: "indent_style",
  indentWidth: "indent_width",
  lineEnding: "line_ending",
  commas: "commas",
} satisfies Record<keyof RequiredOptions, string>;

const valueOf = (text: string) => {
  return /^[1-9]\d*$/.test(text) ? Number(text) : text;
};

// Let @nuten/wasm check the ranges and the choices instead of repeating them.
const takes = <Key extends keyof RequiredOptions>(
  key: Key,
  value: string | number,
): value is RequiredOptions[Key] => {
  try {
    format("", { [key]: value } as Options);
    return true;
  } catch (error) {
    if (error instanceof TypeError) return false;

    throw error;
  }
};

const optionIn = <Key extends keyof RequiredOptions>(params: URLSearchParams, key: Key) => {
  const text = params.get(optionNames[key]);
  if (text === null) return defaultOptions[key];

  const value = valueOf(text);
  return takes(key, value) ? value : defaultOptions[key];
};

const optionsIn = (params: URLSearchParams): RequiredOptions => ({
  lineWidth: optionIn(params, "lineWidth"),
  indentStyle: optionIn(params, "indentStyle"),
  indentWidth: optionIn(params, "indentWidth"),
  lineEnding: optionIn(params, "lineEnding"),
  commas: optionIn(params, "commas"),
});

// lz-string decompresses broken input to "" or to garbage, so a source is taken only when it compresses back.
const decompressed = (code: string) => {
  if (code === "") return "";

  try {
    // lz-string reads spaces as "+", and so must the check below.
    const payload = decodeURIComponent(code).replaceAll(" ", "+");
    const source = decompressFromEncodedURIComponent(payload);
    return source && compressToEncodedURIComponent(source) === payload ? source : undefined;
  } catch {
    return undefined;
  }
};

const compressed = (source: string) => {
  return source === "" ? "" : compressToEncodedURIComponent(source);
};

export const sharedIn = (href: string, sample: string): Shared => {
  const { searchParams, hash } = new URL(href);
  const code = hash.startsWith(codePrefix)
    ? decompressed(hash.slice(codePrefix.length))
    : undefined;
  return { source: code ?? sample, options: optionsIn(searchParams) };
};

export const sharedUrlOf = (href: string, { source, options }: Shared, sample: string) => {
  const url = new URL(href);
  const keys = Object.keys(optionNames) as (keyof RequiredOptions)[];
  const changed = keys.filter((key) => options[key] !== defaultOptions[key]);

  url.search = new URLSearchParams(
    changed.map((key) => [optionNames[key], String(options[key])]),
  ).toString();
  url.hash = source === sample && changed.length === 0 ? "" : `${codePrefix}${compressed(source)}`;
  return url.href;
};

export const keepInUrl = (store: PlaygroundStore, sample: string) => {
  let timer: ReturnType<typeof setTimeout> | undefined;

  const write = () => {
    const url = sharedUrlOf(location.href, store.getSnapshot(), sample);
    if (url !== location.href) history.replaceState(history.state, "", url);
  };
  const unsubscribe = store.subscribe(() => {
    clearTimeout(timer);
    timer = setTimeout(write, urlDelay);
  });

  // The editors read the code only once, so reload when the hash changes.
  const reload = () => location.reload();
  addEventListener("hashchange", reload);

  return () => {
    clearTimeout(timer);
    unsubscribe();
    removeEventListener("hashchange", reload);
  };
};
