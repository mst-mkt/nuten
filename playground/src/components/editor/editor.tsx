import { EditorState } from "@codemirror/state";
import { EditorView, gutters } from "@codemirror/view";
import type { ParseError } from "@nuten/wasm";
import CodeMirror, { type ReactCodeMirrorRef } from "@uiw/react-codemirror";
import type { FC, Ref } from "react";

import { keepCarriageReturns, showCarriageReturns } from "./carriage-returns";
import { syntaxErrors } from "./diagnostics";
import styles from "./editor.module.css";
import { theme } from "./theme";

const scrollingGutters = gutters({ fixed: false });

type EditorProps = {
  ref?: Ref<ReactCodeMirrorRef>;
  labelledBy: string;
  value: string;
  tabSize: number;
  readOnly?: boolean;
  errors?: readonly ParseError[];
  onChange?: (value: string) => void;
};

export const Editor: FC<EditorProps> = ({
  ref,
  labelledBy,
  value,
  tabSize,
  readOnly = false,
  errors = [],
  onChange,
}) => (
  <CodeMirror
    ref={ref}
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
      syntaxErrors(value, errors),
    ]}
    onChange={(next) => onChange?.(next)}
  />
);
