import { resolve } from "node:path";
import { defineConfig } from "@vscode/test-cli";

// One launch per kind of file, since only the first file opened tells which event activates the extension.
const launch = (first) => ({
  label: `first ${first}`,
  files: "out/test/*.test.cjs",
  useInstallation: { fromPath: process.env.NUTEN_VSCODE },
  launchArgs: ["--disable-extensions"],
  env: {
    PATH: `${resolve(import.meta.dirname, "../../target/debug")}:${process.env.PATH}`,
    NUTEN_TEST_FIRST: first,
  },
  mocha: { ui: "tdd", timeout: 20_000 },
});

export default defineConfig([launch(".nu"), launch(".nuon")]);
