import { type RequiredOptions, defaultOptions } from "@nuten/wasm";

import { type FormatResult, formatResultOf } from "./format";

export type Playground = {
  source: string;
  options: RequiredOptions;
  result: FormatResult;
};

export type PlaygroundStore = {
  getSnapshot: () => Playground;
  subscribe: (listener: () => void) => () => void;
  setSource: (source: string) => void;
  setOptions: (options: RequiredOptions) => void;
};

const playgroundOf = (source: string, options: RequiredOptions): Playground => {
  return { source, options, result: formatResultOf(source, options) };
};

export const createPlaygroundStore = (source: string): PlaygroundStore => {
  const listeners = new Set<() => void>();
  let snapshot = playgroundOf(source, defaultOptions);

  const update = (next: Playground) => {
    snapshot = next;
    listeners.forEach((listener) => listener());
  };

  return {
    getSnapshot: () => snapshot,
    subscribe: (listener) => {
      listeners.add(listener);
      return () => listeners.delete(listener);
    },
    setSource: (next) => update(playgroundOf(next, snapshot.options)),
    setOptions: (next) => update(playgroundOf(snapshot.source, next)),
  };
};
