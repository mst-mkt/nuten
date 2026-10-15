import type { EditorView } from "@codemirror/view";
import { useId, useState, useSyncExternalStore } from "react";

import styles from "./app.module.css";
import { CopyButton } from "./components/copy-button/copy-button";
import { InputEditor } from "./components/editor/input-editor";
import { OutputEditor } from "./components/editor/output-editor";
import { revealPosition } from "./components/editor/position";
import { Header } from "./components/header/header";
import { Messages } from "./components/messages/messages";
import { Pane } from "./components/pane/pane";
import { Settings } from "./components/settings/settings";
import { type Position, messagesOf } from "./lib/format";
import { createPlaygroundStore } from "./lib/playground";
import sample from "./lib/sample.nu?raw";

export const App = () => {
  const [store] = useState(() => createPlaygroundStore(sample));
  const { options, result } = useSyncExternalStore(store.subscribe, store.getSnapshot);
  const inputTitleId = useId();
  const outputTitleId = useId();
  const [inputView, setInputView] = useState<EditorView | null>(null);

  const revealInInput = (position: Position) => {
    if (inputView !== null) revealPosition(inputView, position);
  };

  return (
    <div className={styles.app}>
      <Header />
      <main className={styles.main}>
        <Settings options={options} onChange={store.setOptions} />
        <div className={styles.panes}>
          <Pane titleId={inputTitleId} title="Input">
            <InputEditor store={store} labelledBy={inputTitleId} onView={setInputView} />
          </Pane>
          <Pane
            titleId={outputTitleId}
            title="Output"
            actions={result.kind === "output" && <CopyButton text={result.output} />}
          >
            {result.kind === "output" ? (
              <OutputEditor store={store} labelledBy={outputTitleId} />
            ) : (
              <Messages messages={messagesOf(result)} onSelect={revealInInput} />
            )}
          </Pane>
        </div>
      </main>
    </div>
  );
};
