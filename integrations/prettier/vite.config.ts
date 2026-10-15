import { defineConfig } from "vite-plus";

export default defineConfig({
  pack: {
    entry: ["src/index.ts"],
    format: ["esm"],
    dts: true,
    platform: "neutral",
  },
  test: {
    include: ["src/**/*.test.ts"],
  },
  run: {
    tasks: {
      build: {
        command: "vp pack",
        dependsOn: ["@nuten/format#build"],
      },
      test: {
        command: "vp test run",
        dependsOn: ["build"],
        output: [],
      },
    },
  },
});
