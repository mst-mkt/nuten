import { type TokenType, highlight as highlightInWasm } from "./gen/nuten.js";
import { checkSource } from "./source";

export type { TokenType };

export type Highlight = {
  start: number;
  end: number;
  type: TokenType;
};

export const highlight = (source: string): Highlight[] => {
  checkSource(source);

  const highlights = highlightInWasm(source);
  try {
    return highlights.map(({ start, end, type }) => ({ start, end, type }));
  } finally {
    highlights.forEach((highlight) => highlight.free());
  }
};
