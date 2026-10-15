import { EditorState } from "@codemirror/state";
import { EditorView, gutters } from "@codemirror/view";
import CodeMirror from "@uiw/react-codemirror";
import type { FC } from "react";

import { keepCarriageReturns, showCarriageReturns } from "./carriage-returns";
import styles from "./editor.module.css";
import { theme } from "./theme";

const scrollingGutters = gutters({ fixed: false });

type EditorProps = {
  labelledBy: string;
  value: string;
  tabSize: number;
  readOnly?: boolean;
  onChange?: (value: string) => void;
};

export const Editor: FC<EditorProps> = ({
  labelledBy,
  value,
  tabSize,
  readOnly = false,
  onChange,
}) => (
  <CodeMirror
    className={styles.editor}
    height="100%"
    theme={theme}
    value={value}
    readOnly={readOnly}
    basicSetup={{
      highlightSpecialChars: false,
      highlightActiveLine: !readOnly,
      highlightActiveLineGutter: !readOnly,
      foldGutter: false,
      autocompletion: false,
      closeBrackets: false,
      syntaxHighlighting: false,
    }}
    extensions={[
      keepCarriageReturns,
      showCarriageReturns,
      scrollingGutters,
      EditorState.tabSize.of(tabSize),
      EditorView.contentAttributes.of({ "aria-labelledby": labelledBy }),
    ]}
    onChange={(next) => onChange?.(next)}
  />
);
