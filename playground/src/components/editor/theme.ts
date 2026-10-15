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
  ".cm-errorLine": {
    background: "var(--nuten-error-tint)",
  },
  ".cm-errorLine.cm-activeLine": {
    background: "var(--nuten-error-tint-strong)",
  },
  ".cm-gutterElement.cm-errorLineNumber": {
    color: "var(--nuten-error)",
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
  ".cm-nu-comment": {
    color: "var(--nuten-syntax-comment)",
  },
  ".cm-nu-keyword, .cm-nu-operator": {
    color: "var(--nuten-syntax-keyword)",
  },
  ".cm-nu-function": {
    color: "var(--nuten-syntax-function)",
  },
  ".cm-nu-variable, .cm-nu-parameter": {
    color: "var(--nuten-syntax-variable)",
  },
  ".cm-nu-flag, .cm-nu-number, .cm-nu-type, .cm-nu-property": {
    color: "var(--nuten-syntax-constant)",
  },
  ".cm-nu-string": {
    color: "var(--nuten-syntax-string)",
  },
  ".cm-specialChar": {
    color: faint,
  },
  ".cm-visibleTab": {
    position: "relative",
  },
  ".cm-visibleTab::before": {
    content_fallback: '"→"',
    content: '"→" / ""',
    position: "absolute",
    left: 0,
    color: faint,
    pointerEvents: "none",
  },
  ".cm-lintPoint::after": {
    left: "-4px",
    bottom: "-3px",
    borderInline: "4px solid transparent",
    borderBottom: "5px solid var(--nuten-error)",
  },
  ".cm-tooltip": {
    ...surface,
    fontFamily: "var(--nuten-font-sans)",
    borderRadius: "var(--nuten-radius-block)",
    boxShadow: "var(--nuten-shadow-popover)",
    overflow: "hidden",
  },
  ".cm-tooltip-lint": {
    maxWidth: "min(28rem, calc(100vw - 2rem))",
  },
  ".cm-diagnostic": {
    display: "flex",
    gap: "0.5rem",
    alignItems: "start",
    margin: 0,
    padding: "0.5rem 0.75rem",
    fontSize: "var(--nuten-font-size-sm)",
    lineHeight: "1.5",
    border: "none",
  },
  ".cm-diagnostic + .cm-diagnostic": {
    borderTop: "1px solid var(--nuten-rule)",
  },
  ".cm-diagnostic-error::before": {
    content: '""',
    flex: "none",
    width: "0.5rem",
    height: "0.5rem",
    background: "var(--nuten-error)",
    marginBlock: "calc((1lh - 0.5rem) / 2)",
    borderRadius: "50%",
  },
  ".cm-panels": {
    ...surface,
    color: "var(--nuten-text)",
  },
});
