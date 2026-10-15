import { type EditorState, StateField } from "@codemirror/state";
import { Decoration, type DecorationSet, EditorView } from "@codemirror/view";
import { type TokenType, highlight } from "@nuten/highlight";

const markOf = (type: TokenType) => Decoration.mark({ class: `cm-nu-${type}` });

const highlightsOf = (state: EditorState) => {
  const text = state.doc.toString();
  if (!text.isWellFormed()) return Decoration.none;

  const ranges = highlight(text).map(({ start, end, type }) => markOf(type).range(start, end));
  return Decoration.set(ranges);
};

export const highlights = StateField.define<DecorationSet>({
  create: highlightsOf,
  update: (decorations, tr) => (tr.docChanged ? highlightsOf(tr.state) : decorations),
  provide: (field) => EditorView.decorations.from(field),
});
