import { execFile } from "node:child_process";
import { fileURLToPath } from "node:url";
import { promisify } from "node:util";

import { expect, test } from "vite-plus/test";

const run = promisify(execFile);
const root = fileURLToPath(new URL("..", import.meta.url));

test("node_takes_node_entry_by_package_name_without_warnings", async () => {
  const script = `
    import { format } from "@nuten/wasm";
    process.stdout.write(format("ls|sort"));
  `;
  const { stdout, stderr } = await run(
    process.execPath,
    ["--input-type=module", "--eval", script],
    { cwd: root },
  );

  expect(stdout).toBe("ls | sort\n");
  expect(stderr).toBe("");
});

test("default_entry_formats_in_node", async () => {
  const script = `
    import { format } from "./dist/default.js";
    process.stdout.write(format("ls|sort"));
  `;
  const { stdout } = await run(process.execPath, ["--input-type=module", "--eval", script], {
    cwd: root,
  });

  expect(stdout).toBe("ls | sort\n");
});

test("workerd_condition_takes_workerd_entry_over_node_entry", async () => {
  const script = `process.stdout.write(import.meta.resolve("@nuten/wasm"));`;
  const { stdout } = await run(
    process.execPath,
    ["--conditions=workerd", "--input-type=module", "--eval", script],
    { cwd: root },
  );

  expect(stdout).toBe(new URL("../dist/workerd.js", import.meta.url).href);
});

test("workerd_entry_formats_in_workerd", async () => {
  const workerd = fileURLToPath(import.meta.resolve("workerd/bin/workerd"));
  const config = fileURLToPath(new URL("workerd/config.capnp", import.meta.url));

  await run(process.execPath, [workerd, "test", config]);
});
