import { getIndentUnit, indentService } from "@codemirror/language";

import { tokensAt } from "./highlights";

const openers = new Set(["(", "[", "{"]);

export const indentation = indentService.of((context, pos) => {
  const { state } = context;
  const line = state.doc.lineAt(pos);
  const before = state.sliceDoc(line.from, pos).trimEnd();
  const last = before.at(-1);
  if (last === undefined || !openers.has(last)) return null;

  const at = line.from + before.length - 1;
  const tokens = tokensAt(state, at);
  const commentedOrQuoted = tokens.some(({ to, type }) => {
    const text = type === "string" || type === "comment";
    return text && at < to;
  });
  if (commentedOrQuoted) return null;

  const lineIndent = context.lineIndent(line.from, -1);
  return lineIndent + getIndentUnit(state);
});
