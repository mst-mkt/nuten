import { defineConfig } from "vite-plus";

export default defineConfig({
  pack: {
    entry: ["src/extension.ts"],
    format: ["cjs"],
    platform: "node",
    deps: {
      neverBundle: ["vscode"],
    },
    minify: true,
  },
  run: {
    tasks: {
      build: {
        command: "vp pack",
      },
      package: {
        command: "vsce package --no-dependencies --out dist/",
        dependsOn: ["build"],
        input: [{ auto: true }, "!dist/*.vsix"],
      },
    },
  },
});
