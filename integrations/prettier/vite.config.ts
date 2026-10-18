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
      "publish:jsr": {
        command: "vp dlx jsr publish",
        dependsOn: [{ task: "publish:jsr", from: "dependencies" }],
        cache: false,
      },
      test: {
        command: "vp test run",
        dependsOn: ["build"],
        cache: {
          output: [],
        },
      },
    },
  },
});
