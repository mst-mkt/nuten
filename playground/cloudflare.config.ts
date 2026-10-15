import { defineConfig, triggers } from "cf/config";

export default defineConfig({
  worker: {
    name: "nuten-playground",
    compatibilityDate: "2026-10-06",
    triggers: [
      triggers.fetch({ pattern: "nuten.keito.dev/playground", zone: "keito.dev" }),
      triggers.fetch({ pattern: "nuten.keito.dev/playground/", zone: "keito.dev" }),
      triggers.fetch({ pattern: "nuten.keito.dev/playground/assets/*", zone: "keito.dev" }),
    ],
  },
});
