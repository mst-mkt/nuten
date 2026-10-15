import { type Diagnostic, type LintSource, linter, setDiagnosticsEffect } from "@codemirror/lint";
import {
  type EditorState,
  RangeSet,
  type RangeValue,
  StateField,
  type Text,
  type Transaction,
} from "@codemirror/state";
import { Decoration, EditorView, GutterMarker, gutterLineClass } from "@codemirror/view";
import type { ParseError } from "@nuten/format";

import { textOf } from "./line-endings";
import { offsetOf } from "./position";

export type SyntaxErrors = {
  source: string;
  errors: readonly ParseError[];
};

const errorLine = Decoration.line({ class: "cm-errorLine" });

const errorLineNumber = new (class extends GutterMarker {
  override elementClass = "cm-errorLineNumber";
})();

export const diagnosticsOf = (
  state: EditorState,
  { source, errors }: SyntaxErrors,
): Diagnostic[] => {
  if (textOf(state) !== source) return [];

  return errors.map(({ line, column, message }) => {
    const at = offsetOf(state.doc, { line, column });
    return { from: at, to: at, severity: "error", message };
  });
};

export const errorLineStartsOf = (doc: Text, positions: readonly number[]) => [
  ...new Set(positions.map((pos) => doc.lineAt(pos).from)),
];

const diagnosedPositionsOf = (tr: Transaction) => {
  const diagnosed = tr.effects.filter((effect) => effect.is(setDiagnosticsEffect));
  const latest = diagnosed.at(-1);
  return latest?.value.map(({ from }) => from);
};

const markedAt = <Value extends RangeValue>(value: Value) => {
  return (starts: readonly number[]) => {
    const ranges = starts.map((from) => value.range(from));
    return RangeSet.of(ranges, true);
  };
};

const errorLines = StateField.define<readonly number[]>({
  create: () => [],
  update: (starts, tr) => {
    const diagnosed = diagnosedPositionsOf(tr);
    if (diagnosed !== undefined) return errorLineStartsOf(tr.newDoc, diagnosed);
    if (!tr.docChanged) return starts;

    const mapped = starts.map((pos) => tr.changes.mapPos(pos));
    return errorLineStartsOf(tr.newDoc, mapped);
  },
  provide: (field) => [
    EditorView.decorations.from(field, markedAt(errorLine)),
    gutterLineClass.from(field, markedAt(errorLineNumber)),
  ],
});

export const syntaxErrors = (read: () => SyntaxErrors) => {
  const source: LintSource = ({ state }) => diagnosticsOf(state, read());
  const markers = linter(source, { delay: 0 });
  return [markers, errorLines];
};
