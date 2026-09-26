import { resolve } from "node:path";
import { defineConfig } from "@vscode/test-cli";

export default defineConfig({
  files: "out/test/*.test.cjs",
  useInstallation: { fromPath: process.env.NUTEN_VSCODE },
  launchArgs: ["--disable-extensions"],
  env: { PATH: `${resolve(import.meta.dirname, "../../target/debug")}:${process.env.PATH}` },
  mocha: { ui: "tdd", timeout: 20_000 },
});
