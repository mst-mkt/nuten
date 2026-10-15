import { EditorState } from "@codemirror/state";
import type { EditorView } from "@codemirror/view";
import { type FC, useCallback } from "react";

import type { FormatResult } from "../../lib/format";
import type { Playground, PlaygroundStore } from "../../lib/playground";
import { attachEditor } from "./attach";
import styles from "./editor.module.css";

type OutputEditorProps = {
  store: PlaygroundStore;
  labelledBy: string;
};

const outputOf = (result: FormatResult) => {
  return result.kind === "output" ? result.output : "";
};

const followOutput = (view: EditorView, { result }: Playground) => {
  if (result.kind !== "output") return;
  if (result.output === view.state.doc.toString()) return;

  const whole = { from: 0, to: view.state.doc.length };
  view.dispatch({ changes: { ...whole, insert: result.output } });
};

export const OutputEditor: FC<OutputEditorProps> = ({ store, labelledBy }) => {
  const attach = useCallback(
    (parent: HTMLDivElement) => {
      const { detach } = attachEditor(parent, store, {
        doc: outputOf(store.getSnapshot().result),
        labelledBy,
        extensions: EditorState.readOnly.of(true),
        follow: followOutput,
      });
      return detach;
    },
    [store, labelledBy],
  );

  return <div ref={attach} className={styles.editor} />;
};
