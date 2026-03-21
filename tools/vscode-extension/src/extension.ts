import * as vscode from 'vscode';
import {
  Executable,
  LanguageClient,
  LanguageClientOptions,
  ServerOptions,
  TransportKind,
} from 'vscode-languageclient/node';
import { formatOnSaveEnabled, getCliPath, getServerPath, lintOnSaveEnabled } from './config';

const RUN_DIAGNOSTICS_COMMAND = 'spectra.diagnostics.run';
const LINT_WORKSPACE_COMMAND = 'spectra.lintWorkspace';

let client: LanguageClient | undefined;
let outputChannel: vscode.OutputChannel | undefined;

export async function activate(context: vscode.ExtensionContext): Promise<void> {
  outputChannel = vscode.window.createOutputChannel('Spectra');
  context.subscriptions.push(outputChannel);

  client = await startClient(context, outputChannel);

  context.subscriptions.push(
    vscode.commands.registerCommand(RUN_DIAGNOSTICS_COMMAND, async () => {
      const editor = vscode.window.activeTextEditor;
      if (!editor || editor.document.languageId !== 'spectra') {
        vscode.window.showInformationMessage('Open a Spectra file to run diagnostics.');
        return;
      }

      await client?.sendRequest('workspace/executeCommand', {
        command: RUN_DIAGNOSTICS_COMMAND,
        arguments: [editor.document.uri.toString()],
      });
    })
  );

  context.subscriptions.push(
    vscode.commands.registerCommand(LINT_WORKSPACE_COMMAND, async () => {
      const folders = vscode.workspace.workspaceFolders;
      if (!folders || folders.length === 0) {
        vscode.window.showInformationMessage('Open a workspace to run Spectra lint.');
        return;
      }

      await vscode.window.withProgress(
        {
          location: vscode.ProgressLocation.Notification,
          title: 'Spectra lint',
          cancellable: false,
        },
        async () => {
          await client?.sendRequest('workspace/executeCommand', {
            command: LINT_WORKSPACE_COMMAND,
            arguments: folders.map((folder) => folder.uri.fsPath),
          });
        }
      );
    })
  );

  registerFormatOnSaveHook(context);

  context.subscriptions.push(
    vscode.workspace.onDidChangeConfiguration((event) => {
      if (event.affectsConfiguration('spectra.serverPath')) {
        void restartClient(context);
      }
    })
  );
}

export async function deactivate(): Promise<void> {
  if (client) {
    await client.stop();
    client = undefined;
  }
}

async function startClient(
  context: vscode.ExtensionContext,
  output: vscode.OutputChannel
): Promise<LanguageClient> {
  const serverPath = getServerPath(context);
  const executable: Executable = {
    command: serverPath,
    transport: TransportKind.stdio,
  };

  const serverOptions: ServerOptions = {
    run: executable,
    debug: executable,
  };

  const clientOptions: LanguageClientOptions = {
    documentSelector: [
      { scheme: 'file', language: 'spectra' },
      { scheme: 'untitled', language: 'spectra' },
    ],
    outputChannel: output,
    synchronize: {
      configurationSection: 'spectra',
      fileEvents: vscode.workspace.createFileSystemWatcher('**/*.spectra'),
    },
    initializationOptions: {
      spectra: {
        cliPath: getCliPath(),
        lintOnSave: lintOnSaveEnabled(),
      },
    },
  };

  const nextClient = new LanguageClient(
    'spectra',
    'Spectra Language Server',
    serverOptions,
    clientOptions
  );

  await nextClient.start();
  context.subscriptions.push(nextClient);
  output.appendLine(`Spectra language server started from ${serverPath}`);
  return nextClient;
}

async function restartClient(context: vscode.ExtensionContext): Promise<void> {
  if (!outputChannel) {
    return;
  }

  if (client) {
    await client.stop();
  }

  client = await startClient(context, outputChannel);
}

function registerFormatOnSaveHook(context: vscode.ExtensionContext): void {
  context.subscriptions.push(
    vscode.workspace.onWillSaveTextDocument((event) => {
      if (!formatOnSaveEnabled() || event.document.languageId !== 'spectra') {
        return;
      }

      const editorConfig = vscode.workspace.getConfiguration('editor', event.document.uri);
      const formattingOptions: vscode.FormattingOptions = {
        insertSpaces: editorConfig.get<boolean>('insertSpaces', true),
        tabSize: editorConfig.get<number>('tabSize', 4),
      };

      event.waitUntil(
        vscode.commands.executeCommand<vscode.TextEdit[]>(
          'vscode.executeFormatDocumentProvider',
          event.document.uri,
          formattingOptions
        )
      );
    })
  );
}
