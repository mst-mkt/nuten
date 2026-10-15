import { cloudflare } from "@cloudflare/vite-plugin";
import react from "@vitejs/plugin-react";
import { type Plugin, defineConfig } from "vite-plus";

// Workers serve each asset at its path in the output, so the page goes under `playground/` to answer at `/playground/`.
// `assets.basePath` would make this and `assetsDir` unnecessary, but it is not yet enabled for this account.
const servedUnderPlayground: Plugin = {
  name: "served-under-playground",
  apply: "build",
  enforce: "post",
  applyToEnvironment: (environment) => environment.name === "client",
  generateBundle: (_, bundle) => {
    const html = bundle["index.html"];
    if (html !== undefined) {
      html.fileName = "playground/index.html";
    }
  },
};

export default defineConfig({
  plugins: [react({ compiler: true }), cloudflare(), servedUnderPlayground],
  build: {
    assetsDir: "playground/assets",
    rolldownOptions: {
      output: {
        codeSplitting: {
          groups: [
            { name: "react", test: /[\\/]node_modules[\\/](react|react-dom|scheduler)[\\/]/ },
            { name: "base-ui", test: /[\\/]node_modules[\\/](@base-ui|@floating-ui)[\\/]/ },
            {
              name: "codemirror",
              test: /[\\/]node_modules[\\/](@codemirror|@lezer|style-mod|w3c-keyname|crelt)[\\/]/,
            },
          ],
        },
      },
    },
  },
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
