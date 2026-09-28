import { strict as assert } from "node:assert";
import { randomUUID } from "node:crypto";
import { mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { setTimeout } from "node:timers/promises";
import { ConfigurationTarget, Uri, commands, extensions, window, workspace } from "vscode";

const samples = {
  ".nu": { unformatted: "ls  |  length\n", formatted: "ls | length\n" },
  ".nuon": { unformatted: "{a:1}\n", formatted: "{ a: 1 }\n" },
};
const { unformatted, formatted } = samples[".nu"];

// The kind of file this launch opens first, which `.vscode-test.mjs` sets.
const first = (process.env.NUTEN_TEST_FIRST ?? ".nu") as keyof typeof samples;
assert.ok(first in samples, `NUTEN_TEST_FIRST is ${first}`);

const directory = mkdtemp(join(tmpdir(), "nuten-vscode-"));

const nuten = () => {
  const extension = extensions.getExtension("mst-mkt.nuten-vscode");
  assert.ok(extension);

  return extension;
};

const format = async (content: string, extension = ".nu") => {
  const path = join(await directory, `${randomUUID()}${extension}`);
  await writeFile(path, content);

  const document = await workspace.openTextDocument(Uri.file(path));
  await window.showTextDocument(document);
  await commands.executeCommand("editor.action.formatDocument");

  return document.getText();
};

const eventually = async (actual: () => Promise<string>, expected: string) => {
  const deadline = Date.now() + 10_000;

  const poll = async () => {
    const value = await actual();

    if (value === expected || Date.now() > deadline) {
      assert.equal(value, expected);
      return;
    }

    await setTimeout(100);
    return poll();
  };

  await poll();
};

const setPath = (path: string | undefined) => {
  return workspace.getConfiguration("nuten").update("path", path, ConfigurationTarget.Global);
};

suiteTeardown(async () => {
  await rm(await directory, { recursive: true });
});

suite("activation", () => {
  test(`opening_${first.slice(1)}_file_activates_extension_to_format_it`, async () => {
    assert.equal(nuten().isActive, false);

    const sample = samples[first];
    await eventually(() => format(sample.unformatted, first), sample.formatted);
  });
});

suite("formatting", () => {
  suiteSetup(async () => {
    await nuten().activate();
  });

  teardown(async () => {
    await setPath(undefined);
  });

  test("formatting_nu_file_applies_edits_from_nuten", async () => {
    assert.equal(await format(unformatted), formatted);
  });

  test("formatting_nuon_file_applies_edits_from_nuten", async () => {
    const sample = samples[".nuon"];
    assert.equal(await format(sample.unformatted, ".nuon"), sample.formatted);
  });

  test("changing_path_restarts_server_even_after_failed_start", async () => {
    await setPath("nuten-missing");
    await eventually(() => format(unformatted), unformatted);

    await setPath(undefined);
    await eventually(() => format(unformatted), formatted);
  });

  test("path_without_lsp_subcommand_is_not_started_as_server", async () => {
    const path = join(await directory, "nuten-without-lsp");
    const invocations = `${path}.invocations`;
    await writeFile(invocations, "");
    await writeFile(path, `#!/bin/sh\necho "$@" >> '${invocations}'\nexit 2\n`, { mode: 0o755 });

    await setPath(path);
    await eventually(() => readFile(invocations, "utf8"), "lsp --help\n");

    await setPath(undefined);
    await eventually(() => format(unformatted), formatted);
    assert.equal(await readFile(invocations, "utf8"), "lsp --help\n");
  });
});
