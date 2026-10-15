import { CharCategory, type EditorState, Prec, type TransactionSpec } from "@codemirror/state";
import { EditorView, keymap } from "@codemirror/view";
import { highlight } from "@nuten/highlight";

import { type Token, tokensAt } from "./highlights";

const quoteChars = new Set(['"', "'", "`"]);

const emptyPairs = new Set(['""', "''", "``"]);

const isUnclosed = (state: EditorState, token: Token) => {
  if (token.type !== "string" || token.to !== state.doc.length) return false;

  const extended = highlight(`${state.doc.toString()}\n`);
  return extended.some(({ start, end, type }) => {
    const sameString = type === "string" && start === token.from;
    return sameString && end > token.to;
  });
};

const encloses = (state: EditorState, pos: number) => (token: Token) => {
  if (token.from >= pos) return false;
  if (token.type === "comment") return true;
  if (token.type !== "string") return false;

  return pos < token.to || isUnclosed(state, token);
};

const closesAt = (state: EditorState, pos: number) => (token: Token) => {
  const endsAfterPos = token.type === "string" && token.to === pos + 1;
  return endsAfterPos && !isUnclosed(state, token);
};

const opensPairAt = (state: EditorState, pos: number) => {
  const before = state.sliceDoc(pos - 1, pos);
  const after = state.sliceDoc(pos, pos + 1);
  const followsWord = state.charCategorizer(pos)(before) === CharCategory.Word;
  const opensRawString = before === "#";
  const precedesGap = after === "" || /[\s)\]}:;>]/.test(after);
  return !followsWord && !opensRawString && precedesGap;
};

export const quoteTyped = (
  state: EditorState,
  pos: number,
  quote: string,
): TransactionSpec | null => {
  const tokens = tokensAt(state, pos);
  const nextIsQuote = state.sliceDoc(pos, pos + 1) === quote;
  const closing = tokens.some(closesAt(state, pos));
  if (nextIsQuote && closing) {
    return { selection: { anchor: pos + 1 } };
  }

  const inside = tokens.some(encloses(state, pos));
  if (inside || !opensPairAt(state, pos)) return null;

  return { changes: { from: pos, insert: quote + quote }, selection: { anchor: pos + 1 } };
};

export const pairDeleted = (state: EditorState, pos: number): TransactionSpec | null => {
  const around = state.sliceDoc(pos - 1, pos + 1);
  if (!emptyPairs.has(around)) return null;

  const tokens = tokensAt(state, pos);
  const empty = tokens.some(({ from, to, type }) => {
    return type === "string" && from === pos - 1 && to === pos + 1;
  });
  return empty ? { changes: { from: pos - 1, to: pos + 1 } } : null;
};

const cursorOf = (state: EditorState) => {
  const { ranges, main } = state.selection;
  const single = ranges.length === 1 && main.empty;
  return single ? main.head : null;
};

const quoteInput = EditorView.inputHandler.of((view, from, to, text) => {
  if (view.composing || view.compositionStarted) return false;

  const typedQuote = quoteChars.has(text) && from === to;
  if (!typedQuote || cursorOf(view.state) !== from) return false;

  const spec = quoteTyped(view.state, from, text);
  if (spec === null) return false;

  view.dispatch({ ...spec, userEvent: "input.type", scrollIntoView: true });
  return true;
});

const pairBackspace = keymap.of([
  {
    key: "Backspace",
    run: (view) => {
      const pos = cursorOf(view.state);
      if (pos === null) return false;

      const spec = pairDeleted(view.state, pos);
      if (spec === null) return false;

      view.dispatch({ ...spec, userEvent: "delete.backward", scrollIntoView: true });
      return true;
    },
  },
]);

export const quotes = [quoteInput, Prec.high(pairBackspace)];
