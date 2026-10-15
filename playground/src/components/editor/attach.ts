import { EditorState, type Extension } from "@codemirror/state";
import { EditorView } from "@codemirror/view";

import type { Playground, PlaygroundStore } from "../../lib/playground";
import { keepCarriageReturns, showCarriageReturns } from "./carriage-returns";
import { setup } from "./setup";
import { followTabSize, tabSizeOf } from "./tab-size";
import { theme } from "./theme";

type EditorConfig = {
  doc: string;
  labelledBy: string;
  extensions: Extension;
  follow?: (view: EditorView, playground: Playground) => void;
};

export const attachEditor = (parent: HTMLElement, store: PlaygroundStore, config: EditorConfig) => {
  const { options } = store.getSnapshot();

  const extensions = [
    setup,
    keepCarriageReturns,
    showCarriageReturns,
    theme,
    tabSizeOf(options.indentWidth),
    EditorView.contentAttributes.of({ "aria-labelledby": config.labelledBy }),
    config.extensions,
  ];

  const view = new EditorView({
    parent,
    state: EditorState.create({ doc: config.doc, extensions }),
  });

  const unsubscribe = store.subscribe(() => {
    const playground = store.getSnapshot();
    followTabSize(view, playground.options.indentWidth);
    config.follow?.(view, playground);
  });

  const detach = () => {
    unsubscribe();
    view.destroy();
  };

  return { view, detach };
};
