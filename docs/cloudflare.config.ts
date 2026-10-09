import { defineConfig } from "cf/config";

export default defineConfig({
  worker: {
    name: "nuten-docs",
    compatibilityDate: "2026-10-10",
    domains: ["nuten.keito.dev"],
  },
});
