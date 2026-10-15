import type { EditorView } from "@codemirror/view";
import { useEffect, useId, useState, useSyncExternalStore } from "react";

import styles from "./app.module.css";
import { CopyButton } from "./components/copy-button/copy-button";
import { InputEditor } from "./components/editor/input-editor";
import { OutputEditor } from "./components/editor/output-editor";
import { revealPosition } from "./components/editor/position";
import { Header } from "./components/header/header";
import { Messages } from "./components/messages/messages";
import { Pane } from "./components/pane/pane";
import { Settings } from "./components/settings/settings";
import { ShareButton } from "./components/share-button/share-button";
import { Splitter } from "./components/splitter/splitter";
import { type Position, messagesOf } from "./lib/format";
import { createPlaygroundStore } from "./lib/playground";
import sample from "./lib/sample.nu?raw";
import { keepInUrl, sharedIn, sharedUrlOf } from "./lib/share";

export const App = () => {
  const [store] = useState(() => {
    const { source, options } = sharedIn(location.href, sample);
    return createPlaygroundStore(source, options);
  });
  const { options, result } = useSyncExternalStore(store.subscribe, store.getSnapshot);
  const inputTitleId = useId();
  const outputTitleId = useId();
  const inputPaneId = useId();
  const [inputView, setInputView] = useState<EditorView | null>(null);
  const [split, setSplit] = useState(0.5);

  useEffect(() => keepInUrl(store, sample), [store]);

  const revealInInput = (position: Position) => {
    if (inputView !== null) revealPosition(inputView, position);
  };

  return (
    <div className={styles.app}>
      <Header />
      <main className={styles.main}>
        <Settings options={options} onChange={store.setOptions} />
        <div className={styles.panes}>
          <Pane
            id={inputPaneId}
            titleId={inputTitleId}
            title="Input"
            grow={split}
            actions={
              <ShareButton urlOf={() => sharedUrlOf(location.href, store.getSnapshot(), sample)} />
            }
          >
            <InputEditor store={store} labelledBy={inputTitleId} onView={setInputView} />
          </Pane>
          <Splitter
            ratio={split}
            onChange={setSplit}
            label="Resize the input and output"
            controls={inputPaneId}
          />
          <Pane
            titleId={outputTitleId}
            title="Output"
            grow={1 - split}
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
