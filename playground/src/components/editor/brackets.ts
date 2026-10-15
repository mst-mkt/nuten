import { closeBrackets, closeBracketsKeymap } from "@codemirror/autocomplete";
import { EditorState, Prec } from "@codemirror/state";
import { keymap } from "@codemirror/view";

import { tokensAt } from "./highlights";

const inComment = (state: EditorState, pos: number) => {
  const tokens = tokensAt(state, pos);
  return tokens.some(({ from, type }) => {
    return type === "comment" && from < pos;
  });
};

const closingAt = (state: EditorState, pos: number) => {
  const brackets = inComment(state, pos) ? [] : ["(", "[", "{"];
  return [{ closeBrackets: { brackets, before: ")]}:;>\"'`" } }];
};

export const brackets = [
  EditorState.languageData.of(closingAt),
  closeBrackets(),
  Prec.high(keymap.of(closeBracketsKeymap)),
];
