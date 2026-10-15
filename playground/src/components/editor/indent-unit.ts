import { indentUnit } from "@codemirror/language";
import { Compartment } from "@codemirror/state";
import type { EditorView } from "@codemirror/view";
import type { RequiredOptions } from "@nuten/format";

type Indent = Pick<RequiredOptions, "indentStyle" | "indentWidth">;

const unit = new Compartment();

const unitOf = ({ indentStyle, indentWidth }: Indent) => {
  return indentStyle === "tab" ? "\t" : " ".repeat(indentWidth);
};

export const indentUnitOf = (indent: Indent) => {
  const value = unitOf(indent);
  return unit.of(indentUnit.of(value));
};

export const followIndentUnit = (view: EditorView, indent: Indent) => {
  const next = unitOf(indent);
  if (view.state.facet(indentUnit) === next) return;

  const effects = unit.reconfigure(indentUnit.of(next));
  view.dispatch({ effects });
};
