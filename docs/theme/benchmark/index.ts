import { join } from "node:path";
import type { MarkdownNode, MarkdownTransformer } from "@ox-content/vite-plugin";
import { readComparison, type Comparison } from "./comparison";
import { element, render } from "./html";
import { localeOf, type Locale } from "./labels";
import { SECTIONS } from "./sections";

const COMPARISON = join(import.meta.dirname, "../../../assets/benchmark/compare.json");

const isFence = (node: MarkdownNode) => node.type === "code" && node.lang === "benchmark";

const hasFence = (node: MarkdownNode) => isFence(node) || (node.children ?? []).some(hasFence);

const toHtmlNode = (node: MarkdownNode, comparison: Comparison, locale: Locale) => {
  const [name = "", argument] = (node.value ?? "").trim().split(/\s+/);
  const section = SECTIONS.get(name);

  if (section === undefined) {
    throw new Error(`no benchmark section named ${name}`);
  }

  const content = element("div", { class: "benchmark" }, section(comparison, locale, argument));

  return { type: "html", value: render(content) };
};

const visit = (node: MarkdownNode, comparison: Comparison, locale: Locale): MarkdownNode => {
  if (node.children === undefined) return node;

  const children = node.children.map((child) => {
    return isFence(child)
      ? toHtmlNode(child, comparison, locale)
      : visit(child, comparison, locale);
  });

  return { ...node, children };
};

export const benchmark = {
  name: "benchmark",
  transform: (ast, context) => {
    if (!hasFence(ast)) return ast;

    return visit(ast, readComparison(COMPARISON), localeOf(context.filePath));
  },
} satisfies MarkdownTransformer;
