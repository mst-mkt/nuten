import type { MarkdownNode, MarkdownTransformer } from "@ox-content/vite-plugin";

const ESCAPE = String.fromCharCode(0x1b);
const RESET = new RegExp(`${ESCAPE}\\[0?m`);
const SEQUENCE = new RegExp(`${ESCAPE}\\[([\\d;]*)m`, "g");

const CLASSES = {
  "1": "ansi-bold",
  "2": "ansi-dim",
  "31": "ansi-red",
  "32": "ansi-green",
} as const satisfies Record<string, string>;

const ENTITIES = [
  ["&", "&amp;"],
  ["<", "&lt;"],
  [">", "&gt;"],
] as const satisfies [string, string][];

const span = (run: string) => {
  const classes = [...run.matchAll(SEQUENCE)]
    .flatMap(([, codes = ""]) => codes.split(";"))
    .filter((code): code is keyof typeof CLASSES => code in CLASSES)
    .map((code) => CLASSES[code]);

  const text = ENTITIES.reduce(
    (escaped, [char, entity]) => escaped.replaceAll(char, entity),
    run.replace(SEQUENCE, ""),
  );

  return [
    classes.map((name) => `<span class="${name}">`).join(""),
    text,
    "</span>".repeat(classes.length),
  ].join("");
};

const render = (text: string) => {
  return text.split(RESET).map(span).join("");
};

const toHtmlNode = (node: MarkdownNode) => ({
  type: "html",
  value: `<div class="ansi"><code>${render(node.value ?? "")}</code></div>`,
});

const visit = (node: MarkdownNode): MarkdownNode => {
  if (node.children === undefined) return node;

  const children = node.children.map((child) =>
    child.type === "code" && child.lang === "ansi" ? toHtmlNode(child) : visit(child),
  );

  return { ...node, children };
};

export const ansi = {
  name: "ansi",
  transform: (ast) => visit(ast),
} satisfies MarkdownTransformer;
