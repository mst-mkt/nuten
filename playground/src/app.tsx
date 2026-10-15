import { type RequiredOptions, defaultOptions } from "@nuten/wasm";
import type { ReactCodeMirrorRef } from "@uiw/react-codemirror";
import { useId, useRef, useState } from "react";

import styles from "./app.module.css";
import { CopyButton } from "./components/copy-button/copy-button";
import { Editor } from "./components/editor/editor";
import { revealPosition } from "./components/editor/position";
import { Header } from "./components/header/header";
import { Messages } from "./components/messages/messages";
import { Pane } from "./components/pane/pane";
import { Settings } from "./components/settings/settings";
import { type Position, formatResultOf, messagesOf, syntaxErrorsOf } from "./lib/format";
import sample from "./lib/sample.nu?raw";

export const App = () => {
  const [source, setSource] = useState(sample);
  const [options, setOptions] = useState<RequiredOptions>(defaultOptions);
  const result = formatResultOf(source, options);
  const inputTitleId = useId();
  const outputTitleId = useId();
  const inputEditor = useRef<ReactCodeMirrorRef>(null);

  const revealInInput = (position: Position) => {
    const view = inputEditor.current?.view;
    if (view !== undefined) revealPosition(view, position);
  };

  return (
    <div className={styles.app}>
      <Header />
      <main className={styles.main}>
        <Settings options={options} onChange={setOptions} />
        <div className={styles.panes}>
          <Pane titleId={inputTitleId} title="Input">
            <Editor
              ref={inputEditor}
              labelledBy={inputTitleId}
              value={source}
              tabSize={options.indentWidth}
              errors={syntaxErrorsOf(result)}
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
                tabSize={options.indentWidth}
                readOnly
              />
            ) : (
              <Messages messages={messagesOf(result)} onSelect={revealInInput} />
            )}
          </Pane>
        </div>
      </main>
    </div>
  );
};
