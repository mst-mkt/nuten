import { defaultOptions } from "@nuten/wasm";
import { useId, useState } from "react";

import styles from "./app.module.css";
import { CopyButton } from "./components/copy-button/copy-button";
import { Editor } from "./components/editor/editor";
import { Header } from "./components/header/header";
import { Messages } from "./components/messages/messages";
import { Pane } from "./components/pane/pane";
import { formatResultOf, messagesOf } from "./lib/format";
import sample from "./lib/sample.nu?raw";

export const App = () => {
  const [source, setSource] = useState(sample);
  const result = formatResultOf(source, defaultOptions);
  const inputTitleId = useId();
  const outputTitleId = useId();

  return (
    <div className={styles.app}>
      <Header />
      <main className={styles.main}>
        <div className={styles.panes}>
          <Pane titleId={inputTitleId} title="Input">
            <Editor
              labelledBy={inputTitleId}
              value={source}
              tabSize={defaultOptions.indentWidth}
              onChange={setSource}
            />
          </Pane>
          <Pane
            titleId={outputTitleId}
            title="Output"
            actions={result.kind === "output" && <CopyButton text={result.output} />}
          >
            {result.kind === "output" ? (
              <Editor
                labelledBy={outputTitleId}
                value={result.output}
                tabSize={defaultOptions.indentWidth}
                readOnly
              />
            ) : (
              <Messages messages={messagesOf(result)} />
            )}
          </Pane>
        </div>
      </main>
    </div>
  );
};
