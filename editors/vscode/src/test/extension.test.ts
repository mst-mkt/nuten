import { strict as assert } from "node:assert";
import { randomUUID } from "node:crypto";
import { mkdtemp, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { setTimeout } from "node:timers/promises";
import { ConfigurationTarget, Uri, commands, extensions, window, workspace } from "vscode";

const unformatted = "ls  |  length\n";
const formatted = "ls | length\n";

const directory = mkdtemp(join(tmpdir(), "nuten-vscode-"));

const nuten = () => {
  const extension = extensions.getExtension("mst-mkt.nuten-vscode");
  assert.ok(extension);

  return extension;
};

const format = async (content: string) => {
  const path = join(await directory, `${randomUUID()}.nu`);
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
  test("opening_nu_file_activates_extension_to_format_it", async () => {
    assert.equal(nuten().isActive, false);

    await eventually(() => format(unformatted), formatted);
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

  test("changing_path_restarts_server_even_after_failed_start", async () => {
    await setPath("nuten-missing");
    await eventually(() => format(unformatted), unformatted);

    await setPath(undefined);
    await eventually(() => format(unformatted), formatted);
  });
});
