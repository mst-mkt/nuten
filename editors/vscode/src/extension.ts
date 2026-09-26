import { type ExtensionContext, type LogOutputChannel, commands, window, workspace } from "vscode";
import { LanguageClient } from "vscode-languageclient/node";

const reportStartFailure = () => {
  void window
    .showErrorMessage("Failed to start nuten. Check the `nuten.path` setting.", "Open Settings")
    .then((selection) => {
      if (selection !== undefined) {
        void commands.executeCommand("workbench.action.openSettings", "nuten.path");
      }
    });
};

const startClient = async (outputChannel: LogOutputChannel) => {
  const command = workspace.getConfiguration("nuten").get<string>("path", "nuten");
  const client = new LanguageClient(
    "nuten",
    "Nuten",
    { command, args: ["lsp"] },
    { documentSelector: [{ language: "nushell" }], outputChannel },
  );

  await client.start().catch(reportStartFailure);

  return client;
};

const stopClient = async (client: LanguageClient) => {
  if (client.isRunning()) {
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
