import { type Diagnostic, linter } from "@codemirror/lint";
import { type EditorState, RangeSet, type RangeValue, type Text } from "@codemirror/state";
import { Decoration, EditorView, GutterMarker, gutterLineClass } from "@codemirror/view";
import type { ParseError } from "@nuten/wasm";

import { offsetOf } from "./position";

const errorLine = Decoration.line({ class: "cm-errorLine" });

const errorLineNumber = new (class extends GutterMarker {
  override elementClass = "cm-errorLineNumber";
})();

const errorsOf = (doc: Text, source: string, errors: readonly ParseError[]) => {
  return doc.toString() === source ? errors : [];
};

export const diagnosticsOf = (
  doc: Text,
  source: string,
  errors: readonly ParseError[],
): Diagnostic[] => {
  return errorsOf(doc, source, errors).map(({ line, column, message }) => {
    const at = offsetOf(doc, { line, column });
    return { from: at, to: at, severity: "error", message };
  });
};

export const errorLineStartsOf = (doc: Text, source: string, errors: readonly ParseError[]) => {
  const lines = new Set(errorsOf(doc, source, errors).map(({ line }) => line));
  return Array.from(lines, (line) => doc.line(line).from);
};

const errorLinesMarkedBy = <Value extends RangeValue>(
  value: Value,
  source: string,
  errors: readonly ParseError[],
) => {
  return ({ doc }: EditorState) => {
    const starts = errorLineStartsOf(doc, source, errors);
    return RangeSet.of(
      starts.map((from) => value.range(from)),
      true,
    );
  };
};

export const syntaxErrors = (source: string, errors: readonly ParseError[]) => {
  return [
    linter(({ state }) => diagnosticsOf(state.doc, source, errors), { delay: 0 }),
    EditorView.decorations.compute(["doc"], errorLinesMarkedBy(errorLine, source, errors)),
    gutterLineClass.compute(["doc"], errorLinesMarkedBy(errorLineNumber, source, errors)),
  ];
};
