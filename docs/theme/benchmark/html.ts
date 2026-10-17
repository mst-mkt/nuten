type Attributes = Record<string, string | undefined>;

type ElementNode = { tag: string; attributes: Attributes; children: Child[] };

export type Child = ElementNode | string | undefined | Child[];

const ENTITIES = new Map([
  ["&", "&amp;"],
  ["<", "&lt;"],
  [">", "&gt;"],
  ['"', "&quot;"],
]);

const escape = (text: string) => text.replaceAll(/[&<>"]/g, (char) => ENTITIES.get(char) ?? char);

const renderAttributes = (attributes: Attributes) => {
  return Object.entries(attributes)
    .flatMap(([name, value]) => (value === undefined ? [] : [` ${name}="${escape(value)}"`]))
    .join("");
};

export const element = (tag: string, attributes: Attributes, ...children: Child[]) => ({
  tag,
  attributes,
  children,
});

export const render = (child: Child): string => {
  if (child === undefined) return "";
  if (typeof child === "string") return escape(child);
  if (Array.isArray(child)) return child.map(render).join("");

  const { tag, attributes, children } = child;

  return `<${tag}${renderAttributes(attributes)}>${render(children)}</${tag}>`;
};

export const table = (head: string[], rows: Child[][]) => {
  const headCells = head.map((cell) => element("th", { scope: "col" }, cell));
  const bodyRows = rows.map(([first, ...rest]) => {
    const cells = rest.map((cell) => element("td", {}, cell));

    return element("tr", {}, element("th", { scope: "row" }, first), cells);
  });

  return element(
    "div",
    { class: "table-scroll" },
    element(
      "table",
      {},
      element("thead", {}, element("tr", {}, headCells)),
      element("tbody", {}, bodyRows),
    ),
  );
};
