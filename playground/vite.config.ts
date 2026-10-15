import { cloudflare } from "@cloudflare/vite-plugin";
import react from "@vitejs/plugin-react";
import { defineConfig } from "vite-plus";

export default defineConfig({
  base: "./",
  plugins: [react({ compiler: true }), cloudflare()],
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
        input: [{ auto: true }, "!.cloudflare/**"],
      },
      deploy: {
        command: "cf deploy",
        dependsOn: ["@nuten/wasm#build"],
        cache: false,
      },
      test: {
        command: "vp test run",
        dependsOn: ["@nuten/wasm#build"],
        output: [],
      },
    },
  },
});
