import { type TokenType, highlight as highlightInWasm } from "./gen/highlight.js";

export type { TokenType };

export type Highlight = {
  start: number;
  end: number;
  type: TokenType;
};

export const highlight = (source: string): Highlight[] => {
  if (typeof source !== "string") {
    throw new TypeError("`source` takes a string");
  }
  if (!source.isWellFormed()) {
    throw new TypeError("`source` has a lone surrogate");
  }

  const highlights = highlightInWasm(source);
  try {
    return highlights.map(({ start, end, type }) => ({ start, end, type }));
  } finally {
    highlights.forEach((highlight) => highlight.free());
  }
};
