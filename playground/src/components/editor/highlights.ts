import { type EditorState, StateField } from "@codemirror/state";
import { Decoration, EditorView } from "@codemirror/view";
import { type TokenType, highlight } from "@nuten/highlight";

export type Token = {
  from: number;
  to: number;
  type: TokenType;
};

const tokensOf = (state: EditorState): readonly Token[] => {
  const text = state.doc.toString();
  if (!text.isWellFormed()) return [];

  const highlights = highlight(text);
  return highlights.map(({ start, end, type }) => {
    return { from: start, to: end, type };
  });
};

const markOf = ({ from, to, type }: Token) => {
  const mark = Decoration.mark({ class: `cm-nu-${type}` });
  return mark.range(from, to);
};

const decorationsOf = (tokens: readonly Token[]) => {
  const marks = tokens.map(markOf);
  return Decoration.set(marks);
};

export const highlights = StateField.define<readonly Token[]>({
  create: tokensOf,
  update: (tokens, tr) => (tr.docChanged ? tokensOf(tr.state) : tokens),
  provide: (field) => EditorView.decorations.from(field, decorationsOf),
});

export const tokensAt = (state: EditorState, pos: number) => {
  const tokens = state.field(highlights);
  return tokens.filter(({ from, to }) => {
    return from <= pos && pos <= to;
  });
};
