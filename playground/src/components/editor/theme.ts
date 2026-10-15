import { EditorView } from "@codemirror/view";

const surface = {
  background: "var(--nuten-bg-popover)",
  backdropFilter: "blur(var(--nuten-blur))",
  border: "1px solid var(--nuten-border)",
};

const faint = "color-mix(in oklch, var(--nuten-text-muted) 70%, transparent)";

const highlight = "color-mix(in oklch, var(--nuten-primary) 20%, transparent)";

export const theme = EditorView.theme({
  "&": {
    height: "100%",
    color: "var(--nuten-text)",
    background: "transparent",
  },
  "&.cm-focused": {
    outline: "none",
  },
  ".cm-scroller": {
    paddingBlock: "0.75rem",
    fontFamily: "var(--nuten-font-mono)",
    fontSize: "var(--nuten-font-size-sm)",
    lineHeight: "1.7",
  },
  ".cm-gutters": {
    color: faint,
    background: "transparent",
    borderRight: "none",
  },
  ".cm-lineNumbers .cm-gutterElement": {
    minWidth: "3ch",
    paddingInline: "1rem 0.75rem",
  },
  ".cm-activeLine": {
    background: "var(--nuten-tint)",
  },
  ".cm-activeLineGutter": {
    color: "var(--nuten-text)",
    background: "transparent",
  },
  ".cm-cursor, .cm-dropCursor": {
    borderLeft: "2px solid var(--nuten-primary)",
  },
  "&.cm-focused > .cm-scroller > .cm-selectionLayer .cm-selectionBackground, .cm-selectionBackground, .cm-content ::selection":
    {
      background: highlight,
    },
  ".cm-selectionMatch": {
    background: "color-mix(in oklch, var(--nuten-primary) 10%, transparent)",
  },
  ".cm-matchingBracket, &.cm-focused .cm-matchingBracket": {
    background: highlight,
  },
  ".cm-specialChar": {
    color: faint,
  },
  ".cm-panels": {
    ...surface,
    color: "var(--nuten-text)",
  },
});
