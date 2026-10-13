import type { MarkdownNode, MarkdownTransformer } from "@ox-content/vite-plugin";

const open = { type: "html", value: `<div class="table-scroll">` };
const close = { type: "html", value: `</div>` };

const visit = (node: MarkdownNode): MarkdownNode => {
  if (node.children === undefined) return node;

  const children = node.children.flatMap((child) =>
    child.type === "table" ? [open, child, close] : [visit(child)],
  );

  return { ...node, children };
};

export const table = {
  name: "table",
  transform: (ast) => visit(ast),
} satisfies MarkdownTransformer;
