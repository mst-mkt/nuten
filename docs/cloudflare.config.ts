import { defineConfig } from "cf/config";

export default defineConfig({
  worker: ({ isPreview }) => ({
    name: "nuten-docs",
    compatibilityDate: "2026-10-06",
    ...(isPreview ? {} : { domains: ["nuten.keito.dev"] }),
    assets: {
      notFoundHandling: "404-page",
    },
  }),
});
