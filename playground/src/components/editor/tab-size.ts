import { Compartment, EditorState } from "@codemirror/state";
import type { EditorView } from "@codemirror/view";

const tabSize = new Compartment();

export const tabSizeOf = (size: number) => {
  return tabSize.of(EditorState.tabSize.of(size));
};

export const followTabSize = (view: EditorView, size: number) => {
  if (view.state.tabSize === size) return;

  view.dispatch({ effects: tabSize.reconfigure(EditorState.tabSize.of(size)) });
};
