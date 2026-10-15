import { defineConfig } from "vite-plus";

export default defineConfig({
  pack: {
    entry: ["ts/default.ts"],
    format: ["esm"],
    dts: true,
    platform: "neutral",
    unbundle: true,
    deps: {
      neverBundle: [/^node:/, /\.wasm$/],
    },
    copy: [{ from: "ts/gen/highlight_bg.wasm", to: "dist/gen" }],
  },
  test: {
    include: ["ts/**/*.test.ts"],
  },
  run: {
    tasks: {
      "build:wasm": {
        command: "nu build.nu",
        env: ["PATH"],
        untrackedEnv: ["NIX_*", "SDKROOT"],
        input: [
          "build.nu",
          "Cargo.toml",
          "src/**",
          { pattern: "Cargo.{toml,lock}", base: "workspace" },
          { pattern: "crates/**", base: "workspace" },
        ],
        output: ["ts/gen/**"],
      },
      build: {
        command: "vp pack",
        dependsOn: ["build:wasm"],
      },
      test: {
        command: "vp test run",
        dependsOn: ["build"],
        output: [],
      },
    },
  },
});
