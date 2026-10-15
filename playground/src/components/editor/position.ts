import type { Text } from "@codemirror/state";
import type { EditorView } from "@codemirror/view";

import type { Position } from "../../lib/format";

export const offsetOf = (doc: Text, { line, column }: Position) => {
  const { from, text } = doc.line(line);
  const before = Array.from(text)
    .slice(0, column - 1)
    .join("");

  return from + before.length;
};

export const revealPosition = (view: EditorView, position: Position) => {
  view.dispatch({
    selection: { anchor: offsetOf(view.state.doc, position) },
    scrollIntoView: true,
  });
  view.focus();
};
