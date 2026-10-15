import { defaultKeymap, history, historyKeymap, indentWithTab } from "@codemirror/commands";
import { bracketMatching } from "@codemirror/language";
import { lintKeymap } from "@codemirror/lint";
import { highlightSelectionMatches, selectNextOccurrence } from "@codemirror/search";
import { EditorState } from "@codemirror/state";
import {
  drawSelection,
  dropCursor,
  gutters,
  highlightActiveLine,
  highlightActiveLineGutter,
  keymap,
  lineNumbers,
  rectangularSelection,
} from "@codemirror/view";

import { brackets } from "./brackets";

export const setup = [
  lineNumbers(),
  gutters({ fixed: false }),
  history(),
  drawSelection(),
  dropCursor(),
  EditorState.allowMultipleSelections.of(true),
  rectangularSelection(),
  highlightSelectionMatches(),
  bracketMatching(),
  keymap.of([
    ...defaultKeymap,
    { key: "Mod-d", run: selectNextOccurrence, preventDefault: true },
    ...historyKeymap,
    ...lintKeymap,
    indentWithTab,
  ]),
];

export const editableSetup = [highlightActiveLine(), highlightActiveLineGutter(), brackets];
