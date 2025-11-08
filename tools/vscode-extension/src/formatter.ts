import * as vscode from 'vscode';
import { runSpectraCli } from './cliClient';
import { formatOnSaveEnabled, getCliPath } from './config';

function getDocumentWorkspaceFolder(document: vscode.TextDocument): string | undefined {
  return vscode.workspace.getWorkspaceFolder(document.uri)?.uri.fsPath;
}

export class SpectraFormatter implements vscode.DocumentFormattingEditProvider {
  async provideDocumentFormattingEdits(
    document: vscode.TextDocument,
    _options: vscode.FormattingOptions,
    token: vscode.CancellationToken
  ): Promise<vscode.TextEdit[]> {
    const formatted = await this.formatDocument(document, token);
    if (formatted === undefined) {
      return [];
    }

    const fullRange = new vscode.Range(
      0,
      0,
      document.lineCount > 0 ? document.lineCount - 1 : 0,
      document.lineCount > 0
        ? document.lineAt(document.lineCount - 1).text.length
        : 0
    );

    if (formatted === document.getText()) {
      return [];
    }

    return [vscode.TextEdit.replace(fullRange, formatted)];
  }

  async formatDocument(
    document: vscode.TextDocument,
    token: vscode.CancellationToken
  ): Promise<string | undefined> {
    if (document.isUntitled) {
      vscode.window.showInformationMessage(
        'Save the Spectra file before formatting.'
      );
      return undefined;
    }

    const cliPath = getCliPath();
    try {
      const result = await runSpectraCli(['fmt', '--stdin'], {
        cliPath,
        cwd: getDocumentWorkspaceFolder(document),
        token,
        input: document.getText(),
      });

      if (result.exitCode !== 0) {
        vscode.window.showErrorMessage(
          `Spectra formatter exited with code ${result.exitCode}: ${result.stderr.trim()}`
        );
        return undefined;
      }

      return result.stdout;
    } catch (error) {
      vscode.window.showErrorMessage(
        `Failed to run Spectra formatter: ${String(error)}`
      );
      return undefined;
    }
  }
}

export function registerFormatOnSaveHook(
  formatter: SpectraFormatter,
  context: vscode.ExtensionContext
): void {
  context.subscriptions.push(
    vscode.workspace.onWillSaveTextDocument((event) => {
      if (
        !formatOnSaveEnabled() ||
        event.document.languageId !== 'spectra'
      ) {
        return;
      }

      const editorConfig = vscode.workspace.getConfiguration(
        'editor',
        event.document.uri
      );

      const formattingOptions: vscode.FormattingOptions = {
        insertSpaces: editorConfig.get<boolean>('insertSpaces', true),
        tabSize: editorConfig.get<number>('tabSize', 4),
      };

      event.waitUntil(
        (async () => {
          const tokenSource = new vscode.CancellationTokenSource();
          try {
            const edits = await formatter.provideDocumentFormattingEdits(
              event.document,
              formattingOptions,
              tokenSource.token
            );
            return edits.length > 0 ? edits : undefined;
          } catch {
            return undefined;
          } finally {
            tokenSource.dispose();
          }
        })()
      );
    })
  );
}
