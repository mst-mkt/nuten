import { Decoration, MatchDecorator, ViewPlugin } from "@codemirror/view";

const tabDecorator = new MatchDecorator({
  regexp: /\t/g,
  decoration: Decoration.mark({ class: "cm-visibleTab" }),
});

export const visibleTabs = ViewPlugin.define(
  (view) => ({
    decorations: tabDecorator.createDeco(view),
    update(update) {
      this.decorations = tabDecorator.updateDeco(update, this.decorations);
    },
  }),
  { decorations: (plugin) => plugin.decorations },
);
