import * as vscode from 'vscode';
import { SpectraDiagnosticsManager } from './diagnostics';
import { SpectraFormatter, registerFormatOnSaveHook } from './formatter';
import { lintOnSaveEnabled } from './config';
import { getSpectraCliMetadata, resetSpectraCliMetadata } from './cliCapabilities';

let diagnosticsManager: SpectraDiagnosticsManager | undefined;

export function activate(context: vscode.ExtensionContext): void {
  const output = vscode.window.createOutputChannel('Spectra');
  context.subscriptions.push(output);

  diagnosticsManager = new SpectraDiagnosticsManager(context, output);
  diagnosticsManager.attachListeners();
  context.subscriptions.push(diagnosticsManager);

  const formatter = new SpectraFormatter();
  context.subscriptions.push(
    vscode.languages.registerDocumentFormattingEditProvider('spectra', formatter)
  );
  registerFormatOnSaveHook(formatter, context);

  context.subscriptions.push(
    vscode.commands.registerCommand('spectra.diagnostics.run', async () => {
      const editor = vscode.window.activeTextEditor;
      if (!editor || editor.document.languageId !== 'spectra') {
        vscode.window.showInformationMessage('Open a Spectra file to run diagnostics.');
        return;
      }

      await diagnosticsManager?.runDiagnostics(editor.document);
    })
  );

  context.subscriptions.push(
    vscode.commands.registerCommand('spectra.lintWorkspace', async () => {
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
          const files = await vscode.workspace.findFiles('**/*.spectra');
          if (files.length === 0) {
            vscode.window.showInformationMessage('No Spectra files found in workspace.');
            return;
          }

          for (const [index, uri] of files.entries()) {
            const document = await vscode.workspace.openTextDocument(uri);
            await diagnosticsManager?.runDiagnostics(document);
            output.appendLine(`Linted ${uri.fsPath} (${index + 1}/${files.length})`);
          }

          vscode.window.showInformationMessage(
            `Spectra lint completed for ${files.length} file${files.length === 1 ? '' : 's'}.`
          );
        }
      );
    })
  );

  context.subscriptions.push(
    vscode.workspace.onDidChangeConfiguration((event) => {
      if (event.affectsConfiguration('spectra.cliPath')) {
        resetSpectraCliMetadata();
        void logCliMetadata(output);
      }
    })
  );

  if (lintOnSaveEnabled()) {
    // Kick off diagnostics for already open Spectra documents on activation.
    const initialDocs = vscode.workspace.textDocuments.filter(
      (doc: vscode.TextDocument) => doc.languageId === 'spectra'
    );
    for (const doc of initialDocs) {
      void diagnosticsManager?.runDiagnostics(doc);
    }
  }

  void logCliMetadata(output);
}

export function deactivate(): void {
  diagnosticsManager?.dispose();
  diagnosticsManager = undefined;
}

async function logCliMetadata(output: vscode.OutputChannel): Promise<void> {
  const metadata = await getSpectraCliMetadata();
  if (metadata.version) {
    output.appendLine(`Spectra CLI (${metadata.cliPath}) version ${metadata.version}`);
  } else {
    output.appendLine(
      `Spectra CLI (${metadata.cliPath}) version check failed. --version output not available.`
    );
  }
}
