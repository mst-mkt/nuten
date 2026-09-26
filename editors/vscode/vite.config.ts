import { defineConfig } from "vite-plus";

export default defineConfig({
  pack: [
    {
      name: "extension",
      entry: ["src/extension.ts"],
      format: ["cjs"],
      platform: "node",
      deps: {
        neverBundle: ["vscode"],
      },
      minify: true,
      clean: false,
    },
    {
      name: "test",
      entry: ["src/test/*.test.ts"],
      outDir: "out/test",
      format: ["cjs"],
      platform: "node",
      deps: {
        neverBundle: ["vscode"],
      },
    },
  ],
  run: {
    tasks: {
      build: {
        command: "vp pack --filter extension",
      },
      "build:test": {
        command: "vp pack --filter test",
        input: [{ auto: true }, "!out/**"],
      },
      "build:nuten": {
        command: "cargo build --package nuten",
        env: ["PATH"],
        input: [{ auto: true }, { pattern: "!target/**", base: "workspace" }],
        output: [{ pattern: "target/debug/nuten", base: "workspace" }],
      },
      package: {
        command: "vsce package --no-dependencies --out dist/",
        dependsOn: ["build"],
        input: [{ auto: true }, "!dist/*.vsix"],
      },
      test: {
        command: "vscode-test",
        dependsOn: ["build", "build:test", "build:nuten"],
        cache: false,
      },
    },
  },
});
