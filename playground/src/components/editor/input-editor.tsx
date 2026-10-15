import { EditorView } from "@codemirror/view";
import { type FC, useCallback } from "react";

import { syntaxErrorsOf } from "../../lib/format";
import type { PlaygroundStore } from "../../lib/playground";
import { attachEditor } from "./attach";
import { type SyntaxErrors, syntaxErrors } from "./diagnostics";
import styles from "./editor.module.css";
import { textOf } from "./line-endings";
import { editableSetup } from "./setup";

type InputEditorProps = {
  store: PlaygroundStore;
  labelledBy: string;
  onView?: (view: EditorView | null) => void;
};

const syntaxErrorsIn = (store: PlaygroundStore): SyntaxErrors => {
  const { source, result } = store.getSnapshot();
  return { source, errors: syntaxErrorsOf(result) };
};

const sourceSentTo = (store: PlaygroundStore) => {
  return EditorView.updateListener.of((update) => {
    if (update.docChanged) store.setSource(textOf(update.state));
  });
};

export const InputEditor: FC<InputEditorProps> = ({ store, labelledBy, onView }) => {
  const attach = useCallback(
    (parent: HTMLDivElement) => {
      const extensions = [
        editableSetup,
        syntaxErrors(() => syntaxErrorsIn(store)),
        sourceSentTo(store),
      ];
      const text = store.getSnapshot().source;
      const { view, detach } = attachEditor(parent, store, { text, labelledBy, extensions });
      onView?.(view);

      return () => {
        onView?.(null);
        detach();
      };
    },
    [store, labelledBy, onView],
  );

  return <div ref={attach} className={styles.editor} />;
};
