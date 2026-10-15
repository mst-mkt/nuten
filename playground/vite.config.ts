import react from "@vitejs/plugin-react";
import { defineConfig } from "vite-plus";

export default defineConfig({
  plugins: [react({ compiler: true })],
  css: {
    transformer: "lightningcss",
    lightningcss: {
      drafts: { customMedia: true },
    },
  },
  test: {
    include: ["src/**/*.test.ts"],
  },
  run: {
    tasks: {
      "generate:cmk": {
        command: "cmk",
      },
      dev: {
        command: "vp dev",
        dependsOn: ["@nuten/wasm#build"],
        cache: false,
      },
      build: {
        command: "vp build",
        dependsOn: ["@nuten/wasm#build"],
      },
      test: {
        command: "vp test run",
        dependsOn: ["@nuten/wasm#build"],
        output: [],
      },
    },
  },
});
