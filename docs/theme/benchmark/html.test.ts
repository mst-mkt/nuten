import { describe, expect, test } from "vite-plus/test";

import { element, render } from "./html";

describe("render", () => {
  test("text_in_children_and_attributes_is_escaped", () => {
    const text = `<a href="x">&</a>`;

    const rendered = render(element("p", { title: text }, text));

    expect(rendered).toBe(
      `<p title="&lt;a href=&quot;x&quot;&gt;&amp;&lt;/a&gt;">&lt;a href=&quot;x&quot;&gt;&amp;&lt;/a&gt;</p>`,
    );
  });

  test("undefined_attributes_and_children_are_left_out", () => {
    const span = element("span", { class: "a", style: undefined }, undefined, "b");

    const rendered = render(span);

    expect(rendered).toBe(`<span class="a">b</span>`);
  });

  test("nested_elements_and_lists_are_rendered_once", () => {
    const list = element(
      "ul",
      {},
      ["a", "b"].map((item) => element("li", {}, item)),
    );

    const rendered = render(list);

    expect(rendered).toBe("<ul><li>a</li><li>b</li></ul>");
  });
});
