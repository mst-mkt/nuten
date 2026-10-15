import { EditorState, type Extension } from "@codemirror/state";
import { EditorView } from "@codemirror/view";

import type { Playground, PlaygroundStore } from "../../lib/playground";
import { highlights } from "./highlights";
import { followIndentUnit, indentUnitOf } from "./indent-unit";
import { lineEndings, splitLineEndings } from "./line-endings";
import { setup } from "./setup";
import { followTabSize, tabSizeOf } from "./tab-size";
import { theme } from "./theme";
import { visibleTabs } from "./visible-tabs";

type EditorConfig = {
  text: string;
  labelledBy: string;
  extensions: Extension;
  follow?: (view: EditorView, playground: Playground) => void;
};

export const attachEditor = (parent: HTMLElement, store: PlaygroundStore, config: EditorConfig) => {
  const { options } = store.getSnapshot();
  const { doc, crlf } = splitLineEndings(config.text);

  const extensions = [
    setup,
    lineEndings(crlf),
    highlights,
    visibleTabs,
    theme,
    tabSizeOf(options.indentWidth),
    indentUnitOf(options),
    EditorView.contentAttributes.of({ "aria-labelledby": config.labelledBy }),
    config.extensions,
  ];

  const view = new EditorView({
    parent,
    state: EditorState.create({ doc, extensions }),
  });

  const unsubscribe = store.subscribe(() => {
    const playground = store.getSnapshot();
    followTabSize(view, playground.options.indentWidth);
    followIndentUnit(view, playground.options);
    config.follow?.(view, playground);
  });

  const detach = () => {
    unsubscribe();
    view.destroy();
  };

  return { view, detach };
};
