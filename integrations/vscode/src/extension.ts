import { execFile } from "node:child_process";
import { type ExtensionContext, type LogOutputChannel, commands, window, workspace } from "vscode";
import { LanguageClient } from "vscode-languageclient/node";

const reportStartFailure = (message: string) => {
  void window.showErrorMessage(message, "Open Settings").then((selection) => {
    if (selection !== undefined) {
      void commands.executeCommand("workbench.action.openSettings", "nuten.path");
    }
  });
};

const lacksLspSubcommand = (command: string) => {
  return new Promise<boolean>((resolve) => {
    execFile(command, ["lsp", "--help"], { timeout: 5_000 }, (error) => resolve(error?.code === 2));
  }).catch(() => false);
};

const startClient = async (outputChannel: LogOutputChannel) => {
  const command = workspace.getConfiguration("nuten").get<string>("path", "nuten");

  if (await lacksLspSubcommand(command)) {
    reportStartFailure(`\`${command}\` has no \`lsp\` subcommand. Update nuten.`);
    return;
  }

  const client = new LanguageClient(
    "nuten",
    "Nuten",
    { command, args: ["lsp"] },
    { documentSelector: [{ language: "nushell" }, { language: "nuon" }], outputChannel },
  );

  await client
    .start()
    .catch(() => reportStartFailure("Failed to start nuten. Check the `nuten.path` setting."));

  return client;
};

const stopClient = async (client: LanguageClient | undefined) => {
  if (client?.isRunning()) {
    await client.dispose();
  }
};

export const activate = async (context: ExtensionContext) => {
  const outputChannel = window.createOutputChannel("Nuten", { log: true });
  let startedClient = startClient(outputChannel);

  const restartClient = () => {
    startedClient = startedClient.then(stopClient).then(() => startClient(outputChannel));
  };

  context.subscriptions.push(
    outputChannel,
    { dispose: () => startedClient.then(stopClient) },
    workspace.onDidChangeConfiguration((event) => {
      if (event.affectsConfiguration("nuten.path")) {
        restartClient();
      }
    }),
  );

  await startedClient;
};
