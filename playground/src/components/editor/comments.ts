import { EditorState } from "@codemirror/state";

export const comments = EditorState.languageData.of(() => [{ commentTokens: { line: "#" } }]);
