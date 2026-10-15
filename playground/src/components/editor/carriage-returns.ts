import { EditorState } from "@codemirror/state";
import { highlightSpecialChars } from "@codemirror/view";

const carriageReturn = 0x0d;

export const keepCarriageReturns = EditorState.lineSeparator.of("\n");

export const showCarriageReturns = highlightSpecialChars({
  render: (code, description, placeholder) => {
    return Object.assign(document.createElement("span"), {
      className: "cm-specialChar",
      textContent: code === carriageReturn ? "␍" : placeholder,
      title: description ?? "",
    });
  },
});
